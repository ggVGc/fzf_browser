defmodule Fub.Session do
  require Logger
  alias Fub.Source

  def new(client_socket) do
    state = %{
      sources: %{},
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
    prefix = state.current_source.get_query_prefix(source_state)
    key_bindings = state.current_source.get_key_bindings(source_state)
    query = state.current_source.get_query(source_state)
    open_finder(socket, query, prefix, key_bindings)
    content = state.current_source.get_content(source_state)
    :ok = stream_response(socket, content)
    {:ok, state}
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

  # "ctrl-z" ->
  #   list_recent_locations(socket, state)
  #   {:ok, state}

  # defp list_recent_locations(socket, state) do
  #   open_finder(socket, state.stored_query, state.current_directory)
  #   %Porcelain.Process{out: content} = Porcelain.spawn("fasd", ["-ld"], out: :stream)
  #   respond(socket, :begin_entries)

  #   # TODO: Run asynchronously and stop streaming if any command 
  #   # is received from client.
  #   Enum.each(content, fn chunk ->
  #     respond(socket, :raw, chunk)
  #   end)

  #   respond(socket, :end_entries)
  #   respond(socket, :end_of_content)
  #   :ok
  # end

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
        sources: %{
          Source.Filesystem => Source.Filesystem.new(start_directory)
        }
    }

    run_current_source(socket, state)
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

          {:continue, new_source_state} ->
            {:ok,
             state =
               Map.update!(state, :sources, fn states ->
                 Map.put(states, state.current_source, new_source_state)
               end)}

            run_current_source(socket, state)
        end
    end
  end

  defp respond(socket, prefix, content \\ nil) do
    {:ok, payload} = Fub.Protocol.encode(prefix, content)
    :ok = :gen_tcp.send(socket, payload)
  end
end
