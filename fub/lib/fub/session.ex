defmodule Fub.Session do
  require Logger
  alias Fub.Source

  def new(client_socket) do
    state = %{
      sources: %{},
      previous_source: nil,
      current_source: nil
    }

    loop(client_socket, state)
  end

  defp open_finder(socket, query, prompt_prefix, key_bindings) do
    respond(socket, :open_finder, %{
      query: query,
      key_bindings: key_bindings,
      with_ansi_colors: true,
      prompt_prefix: prompt_prefix
    })
  end

  defp current_source_state(state) do
    state.sources[state.current_source]
  end

  defp run_current_source(socket, state) do
    source_state = current_source_state(state)

    %{
      prefix: prefix,
      query: query,
      key_bindings: key_bindings
    } = state.current_source.get_launch_info(source_state)

    open_finder(socket, query, prefix, key_bindings)
    content = state.current_source.get_content(source_state)
    :ok = stream_response(socket, content)
    :ok
  end

  defp stream_response(socket, content) do
    respond(socket, :begin_entries)
    # TODO: Run asynchronously and stop streaming if any command 
    # is received from client.
    Enum.each(content, fn chunk ->
      respond(socket, :raw, chunk)
    end)

    respond(socket, :end_entries)
    respond(socket, :end_of_content)
    :ok
  end

  defp loop(socket, state) do
    Logger.debug("Waiting for message")

    case :gen_tcp.recv(socket, 0) do
      {:ok, message} ->
        {:ok, state} = handle_message(socket, Jason.decode!(message), state)
        loop(socket, state)

      {:error, :closed} ->
        Logger.debug("Connection closed")

      {:error, :enotconn} ->
        Logger.debug("Not connected")
    end
  end

  defp handle_message(socket, %{"tag" => "client_init"} = message, state) do
    start_directory = Map.fetch!(message, "start_directory")
    Logger.debug("Client started in #{start_directory}")

    state = %{
      state
      | current_source: Source.Filesystem,
        previous_source: Source.Filesystem,
        sources: %{
          Source.Filesystem => Source.Filesystem.new(start_directory),
          Source.Recent => Source.Recent.new()
        }
    }

    :ok = run_current_source(socket, state)
    {:ok, state}
  end

  defp handle_message(socket, %{"tag" => "result"} = message, state) do
    case Map.fetch!(message, "code") do
      code when code in [0, 1] ->
        query = Map.fetch!(message, "query")
        selection = Map.fetch!(message, "selection")
        key = Map.fetch!(message, "key")

        result =
          state.current_source.handle_result(
            current_source_state(state),
            selection,
            query,
            key
          )

        case result do
          {:exit, output} ->
            respond(socket, :exit, "'#{output}'")
            {:ok, state}

          {:switch_source, :previous} ->
            state = %{state | current_source: state.previous_source}
            :ok = run_current_source(socket, state)
            {:ok, state}

          {:switch_source, new_source_state} ->
            %source_module{} = new_source_state

            state = %{
              state
              | current_source: source_module,
                previous_source: state.previous_source,
                sources: Map.put(state.sources, source_module, new_source_state)
            }

            :ok = run_current_source(socket, state)
            {:ok, state}

          {:continue, new_source_state} ->
            {:ok,
             state =
               Map.update!(state, :sources, fn states ->
                 Map.put(states, state.current_source, new_source_state)
               end)}

            :ok = run_current_source(socket, state)
            {:ok, state}
        end
    end
  end

  defp respond(socket, prefix, content \\ nil) do
    {:ok, payload} = Fub.Protocol.encode(prefix, content)
    :ok = :gen_tcp.send(socket, payload)
  end
end
