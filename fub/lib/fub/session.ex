defmodule Fub.Session do
  require Logger

  @key_bindings ["ctrl-y", "ctrl-s"]

  def new(client_socket) do
    state = %{
      start_directory: nil,
      current_directory: nil,
      flags: %{
        sort: true,
        recursion_depth: 1,
        show_hidden: false,
        # mode: :files, :directories, :mixed
        mode: :mixed
      },
      cache: %{}
    }

    loop(client_socket, state)
  end

  defp open_finder(socket, query) do
    respond(socket, :open_finder, %{query: query, key_bindings: @key_bindings})
  end

  defp list_dir(socket, path, cache, flags) do
    key = {path, flags.mode}

    cache |> IO.inspect(label: "cache")

    {cache, content} =
      if content = Map.get(cache, key) do
        # TODO: Start task for cache update
        # If new entries are found, push them.
        # If an entry which is removed is selected, error upon selection and refresh.
        # This should be a very uncommon case.
        {cache, content}
      else
        content = File.ls!(path)
        {Map.put(cache, key, content), content}
      end

    content =
      if flags.sort do
        Enum.sort(content)
      else
        content
      end

    Enum.each(content, fn entry ->
      respond(socket, :entry, entry)
    end)

    respond(socket, :end_of_content)
    cache
  end

  defp list_current_dir(socket, state) do
    %{state | cache: list_dir(socket, state.current_directory, state.cache, state.flags)}
  end

  defp loop(socket, state) do
    Logger.debug("Waiting for message")

    case :gen_tcp.recv(socket, 0) do
      {:ok, message} ->
        state = handle_message(socket, Jason.decode!(message), state)
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

    state = %{state | start_directory: start_directory, current_directory: start_directory}

    open_finder(socket, "")
    _state = list_current_dir(socket, state)
  end

  defp handle_message(socket, %{"tag" => "result"} = message, state) do
    case Map.fetch!(message, "code") do
      code when code in [0, 1] ->
        case Map.fetch!(message, "key") do
          "ctrl-s" ->
            state = %{state | flags: %{state.flags | sort: not state.flags.sort}}

            open_finder(socket, "")
            list_current_dir(socket, state)
            state

          "" ->
            respond(socket, :exit, Map.fetch!(message, "output"))
            state
        end
    end
  end

  defp respond(socket, prefix, content \\ nil) do
    {:ok, payload} = Fub.Protocol.encode(prefix, content)
    :ok = :gen_tcp.send(socket, payload)
  end
end
