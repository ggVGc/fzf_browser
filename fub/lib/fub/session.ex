defmodule Fub.Session do
  use GenServer

  require Logger
  alias Fub.Source

  @key_bindings [
    # Toggle sorting
    "ctrl-s"
  ]

  def start_link([client_socket]) do
    Logger.info("Starting session")
    GenServer.start_link(__MODULE__, client_socket, [])
  end

  @impl true
  def init(client_socket) do
    state = %{
      client_socket: client_socket,
      sources: %{},
      launch_directory: "",
      previous_source: nil,
      current_source: nil,
      stream_task: nil,
      flags: %{
        sort: false
      }
    }

    :inet.setopts(client_socket, [{:active, true}])
    Logger.info("Session started")
    {:ok, state}
  end

  @impl true
  def handle_info({:tcp, _socket, message}, state) do
    {:ok, state} = handle_message(Jason.decode!(message), state)
    {:noreply, state}
  end

  def handle_info({_ref, :ok}, state) do
    # %{stream_task: %{ref: ^ref}} = state
    Logger.debug("Streaming ref exited: :ok")
    {:noreply, state}
  end

  def handle_info({:DOWN, _ref, :process, _pid, reason}, state) do
    # %{stream_task: %{ref: ^ref}} = state
    Logger.debug("Streaming ref DOWN, reason: #{inspect(reason)}")
    {:noreply, %{state | stream_task: nil}}
  end

  def handle_info({:tcp_closed, _socket}, state) do
    Logger.debug("Connection closed")
    {:noreply, state}
  end

  def handle_info(:delayed_run_current_source, state) do
    run_current_source(state)
    {:noreply, state}
  end

  def send_response(pid, prefix, content \\ nil) do
    GenServer.call(pid, {:send_response, prefix, content})
  end

  @impl true
  def handle_call({:send_response, prefix, content}, _caller, state) do
    reply = respond(state.client_socket, prefix, content)
    {:reply, reply, state}
  end

  defp open_finder(socket, query, prompt_prefix, flags, key_bindings) do
    :ok =
      respond(socket, :open_finder, %{
        query: query,
        key_bindings: key_bindings ++ @key_bindings,
        with_ansi_colors: true,
        # sort: flags.sort,
        sort: true,
        prompt_prefix:
          if flags.sort do
            "[s]#{prompt_prefix}"
          else
            prompt_prefix
          end
      })
  end

  defp current_source_state(state) do
    state.sources[state.current_source]
  end

  defp run_current_source(%{stream_task: nil} = state) do
    source_state = current_source_state(state)

    %{
      query_prefix: prefix,
      query: query,
      key_bindings: key_bindings
    } = state.current_source.get_launch_info(source_state)

    open_finder(state.client_socket, query, prefix, state.flags, key_bindings)
    content = state.current_source.get_content(source_state)
    content = if state.flags.sort do
      content
      |> Task.async_stream(&String.split(&1, "\n"))
      |> Stream.flat_map(fn {:ok, line} -> 
        line
      end)
      |> Enum.sort()
      |> Stream.drop_while(& &1 == "")
      |> Stream.map(& &1 <> "\n")
    else
      content
    end

    {:ok, task} = stream_response(self(), content)
    state = %{state | stream_task: task}
    {:ok, state}
  end

  defp run_current_source(%{stream_task: task} = state) when not is_nil(task) do
    Logger.debug("Waiting for stream exit")
    Process.send_after(self(), :delayed_run_current_source, 100)
    {:ok, state}
  end

  defp stream_response(parent_pid, content) do
    task =
      Task.Supervisor.async_nolink(Fub.TaskSupervisor, fn ->
        :ok = send_response(parent_pid, :begin_entries)
        # TODO: Run asynchronously and stop streaming if any command 
        # is received from client.
        result =
          try do
            for chunk <- content do
              :ok = send_response(parent_pid, :raw, chunk)
            end

            :ok = send_response(parent_pid, :end_entries)
            :ok = send_response(parent_pid, :end_of_content)
          rescue
            _ -> :aborted
          end

        if result == :ok do
          Logger.debug("Streaming completed")
        else
          Logger.debug("Streaming aborted, result: #{inspect(result)}")
        end
      end)

    {:ok, Map.take(task, [:ref, :pid])}
  end

  defp stop_streaming(%{stream_task: nil}) do
    :ok
  end

  defp stop_streaming(%{stream_task: %{pid: task_pid}}) do
    Logger.debug("Terminating streaming task")
    Task.Supervisor.terminate_child(Fub.TaskSupervisor, task_pid)
    :ok
  end

  defp handle_message(%{"tag" => "client_init"} = message, state) do
    start_directory = Map.fetch!(message, "start_directory")
    launch_directory = Map.fetch!(message, "launch_directory")
    query = Map.get(message, "start_query", "")
    recursive = Map.get(message, "recursive", false)

    Logger.debug("Client started in #{start_directory}")

    state = %{
      state
      | current_source: Source.Filesystem,
        previous_source: Source.Filesystem,
        launch_directory: launch_directory,
        sources: %{
          Source.Filesystem => Source.Filesystem.new(start_directory, query, recursive),
          Source.Recent => Source.Recent.new()
        }
    }

    {:ok, _state} = run_current_source(state)
  end

  defp handle_message(%{"tag" => "result"} = message, state) do
    Logger.info("Received result: #{inspect(message)}")
    :ok = stop_streaming(state)

    case Map.fetch!(message, "code") do
      code when code in [0, 1] ->
        query = Map.fetch!(message, "query")
        selection = Map.fetch!(message, "selection")
        key = Map.fetch!(message, "key")

        result =
          state.current_source.handle_result(current_source_state(state), selection, query, key)

        case key do
          "ctrl-s" ->
            state = %{state | flags: %{state.flags | sort: not state.flags.sort}}
            run_current_source(state)

          _ ->
            case result do
              {:exit, output} ->
                output = Path.relative_to(output, state.launch_directory)

                output =
                  if String.starts_with?(output, "/") do
                    output
                  else
                    "./#{output}"
                  end

                :ok = respond(state.client_socket, :exit, "#{output}")
                {:ok, state}

              {:switch_source, :previous} ->
                state = %{state | current_source: state.previous_source}
                run_current_source(state)

              {:switch_source, new_source_state} ->
                %source_module{} = new_source_state

                state = %{
                  state
                  | current_source: source_module,
                    previous_source: state.previous_source,
                    sources: Map.put(state.sources, source_module, new_source_state)
                }

                run_current_source(state)

              {:continue, new_source_state} ->
                state =
                  Map.update!(state, :sources, fn states ->
                    Map.put(states, state.current_source, new_source_state)
                  end)

                run_current_source(state)
            end
        end
    end
  end

  defp respond(socket, prefix, content) do
    {:ok, payload} = Fub.Protocol.encode(prefix, content)
    :gen_tcp.send(socket, payload)
  end
end
