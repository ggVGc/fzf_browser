defmodule Fub.Session do
  use GenServer

  require Logger
  alias Fub.Source

  @key_bindings [
    # Toggle sorting
    "ctrl-s",
    # Toggle preview
    "ctrl-p"
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
      stream_socket: "",
      previous_source: nil,
      current_source: nil,
      stream_task: nil,
      flags: %{
        sort: true,
        preview: true
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

  def handle_info({:delayed_run_current_source, query}, state) do
    run_current_source(state, query)
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

  defp open_finder(socket, query, prompt_prefix, flags, key_bindings, preview_command) do
    args = %{
      query: query,
      key_bindings: key_bindings ++ @key_bindings,
      with_ansi_colors: true,
      # sort: flags.sort,
      # If the input list is sorted, we're not interested in the fuzzy browser sorting it
      sort: not flags.sort,
      prompt_prefix:
        if flags.sort do
          "[s]#{prompt_prefix}"
        else
          prompt_prefix
        end
    }

    args =
      if flags.preview and preview_command do
        Map.put(args, :preview_command, preview_command)
      else
        args
      end

    :ok = respond(socket, :open_finder, args)
  end

  defp current_source_state(state) do
    state.sources[state.current_source]
  end

  defp run_current_source(state, query \\ nil)

  defp run_current_source(%{stream_task: nil} = state, query) do
    source_state = current_source_state(state)

    %{
      query_prefix: prefix,
      query: source_query,
      key_bindings: key_bindings
    } = state.current_source.get_launch_info(source_state)

    query = if(query, do: query, else: source_query)

    open_finder(
      state.client_socket,
      query,
      prefix,
      state.flags,
      key_bindings,
      state.current_source.get_preview_command(source_state)
    )

    content = state.current_source.get_content(source_state)

    content =
      if state.flags.sort do
        content
        # |> Task.async_stream(&String.split(&1, "\n"))
        # |> Stream.flat_map(fn {:ok, line} ->
        #   line
        # end)
        |> Enum.join()
        |> String.split("\n")
        |> Enum.sort()
        # First element is "" because of line splitting
        |> Enum.drop(1)
        |> Stream.map(&(&1 <> "\n"))
      else
        content
      end

    {:ok, task} = stream_response(state, content)
    state = %{state | stream_task: task}
    {:ok, state}
  end

  defp run_current_source(%{stream_task: task} = state, query) when not is_nil(task) do
    Logger.debug("Waiting for stream exit")
    Process.send_after(self(), {:delayed_run_current_source, query}, 100)
    {:ok, state}
  end

  defp stream_response(state, content) do
    task =
      Task.Supervisor.async_nolink(Fub.TaskSupervisor, fn ->
        # TODO: Run asynchronously and stop streaming if any command 
        # is received from client.
        {:ok, socket} = :gen_tcp.connect({:local, state.stream_socket}, 0, [])

        result =
          try do
            for chunk <- content do
              :ok = :gen_tcp.send(socket, chunk)
            end

            :ok = :gen_tcp.close(socket)
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
    file_mode = Map.get(message, "file_mode")
    stream_socket = Map.get(message, "stream_socket")

    Logger.debug("Client started in #{start_directory}")

    state = %{
      state
      | current_source: Source.Filesystem,
        previous_source: Source.Filesystem,
        launch_directory: launch_directory,
        stream_socket: stream_socket,
        sources: %{
          Source.Filesystem => Source.Filesystem.new(start_directory, query, recursive, file_mode),
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
            run_current_source(state, query)

          "ctrl-p" ->
            state = %{state | flags: %{state.flags | preview: not state.flags.preview}}
            run_current_source(state, query)

          _ ->
            case result do
              {:exit, entries} when is_list(entries) ->
                response =
                  entries
                  |> Enum.map(&decorate_entry(&1, state))
                  |> Enum.join(" ")

                :ok = respond(state.client_socket, :exit, response)
                {:ok, state}

              {:exit, entry} ->
                :ok = respond(state.client_socket, :exit, decorate_entry(entry, state))
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

  defp decorate_entry(entry, state) do
    entry = Path.relative_to(entry, state.launch_directory)

    entry =
      if String.starts_with?(entry, "/") do
        entry
      else
        "./#{entry}"
      end

    if String.match?(entry, ~r|^[[:alnum:]-._/]+$|) do
      entry
    else
      "'#{entry}'"
    end
  end

  defp respond(socket, prefix, content) do
    {:ok, payload} = Fub.Protocol.encode(prefix, content)
    :gen_tcp.send(socket, payload)
  end
end
