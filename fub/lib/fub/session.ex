defmodule Fub.Session do
  require Logger

  @key_bindings [
    # Go into directory, or open file
    "right",
    # Go up one directory
    "left",
    # Dir stack back
    "ctrl-o",
    # Dir stack forward
    "ctrl-u",
    # Toggle sorting
    "ctrl-y",
    "ctrl-s",
    # Go to home directory
    "ctrl-h",
    # Launch directory jumper (currently fasd -ld)
    "ctrl-z",
    # Go to directory of selected file
    "ctrl-g",
    # Toggle hidden files
    "ctrl-a",
    "\\"
  ]

  def new(client_socket) do
    state = %{
      start_directory: nil,
      current_directory: nil,
      stored_query: "",
      flags: %{
        sort: true,
        recursive: false,
        show_hidden: false,
        # mode: :files, :directories, :mixed
        mode: :mixed
      },
      cache: %{}
    }

    loop(client_socket, state)
  end

  defp open_finder(socket, query) do
    respond(socket, :open_finder, %{
      query: query,
      key_bindings: @key_bindings,
      with_ansi_colors: true
    })
  end

  defp list_dir(socket, path, flags) do
    fd_args =
      List.flatten([
        ["--color=always"],
        if(flags.recursive, do: [], else: ["--max-depth=1"]),
        if(flags.show_hidden, do: ["-H"], else: []),
        case flags.mode do
          :directories ->
            ["--type", "d"]

          :files ->
            ["--type", "f"]

          :mixed ->
            []
        end
      ])
      |> IO.inspect(label: "fd_args")

    # TODO: Stream instead of buffering whole output. Maybe use porcelain.
    {content, 0} = System.cmd("fd", fd_args, cd: path)

    content = String.split(content, "\n")

    content =
      if flags.sort do
        Enum.sort(content)
      else
        content
      end

    Enum.each(content, fn entry ->
      if entry != "" do
        respond(socket, :entry, entry)
      end
    end)

    respond(socket, :end_of_content)
    :ok
  end

  defp list_current_dir(socket, state) do
    open_finder(socket, state.stored_query)
    list_dir(socket, state.current_directory, state.flags)
    state
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

    _state = list_current_dir(socket, state)
  end

  defp handle_message(socket, %{"tag" => "result"} = message, state) do
    case Map.fetch!(message, "code") do
      code when code in [0, 1] ->
        case Map.fetch!(message, "key") do
          key when key in @key_bindings ->
            query = Map.fetch!(message, "query")
            state = handle_key(key, state)
            state = %{state | stored_query: query}
            list_current_dir(socket, state)

          "" ->
            selection = Map.fetch!(message, "selection")
            handle_selection(socket, selection, state)

          tag ->
            Logger.error("Unhandled message tag: #{tag}")
            list_current_dir(socket, state)
        end
    end
  end

  defp handle_selection(socket, selection, state) do
    full_path = Path.join(state.current_directory, selection)

    if File.dir?(full_path) do
      state = %{state | current_directory: full_path}
      list_current_dir(socket, state)
    else
      result = Path.relative_to(full_path, state.start_directory)
      # Only quote result of selection contains non-alphanumeric/period characters.
      respond(socket, :exit, "'#{result}'")
      state
    end
  end

  defp handle_key(key, state) do
    case key do
      sort_toggle when sort_toggle in ["ctrl-s", "ctrl-y"] ->
        Logger.debug("Toggling sort: #{not state.flags.sort}")
        %{state | flags: %{state.flags | sort: not state.flags.sort}}

      "ctrl-a" ->
        %{state | flags: %{state.flags | show_hidden: not state.flags.show_hidden}}

      "\\" ->
        %{state | flags: %{state.flags | recursive: not state.flags.recursive}}

      _ ->
        Logger.error("Unhandled key: #{key}")
        state
    end
  end

  defp respond(socket, prefix, content \\ nil) do
    {:ok, payload} = Fub.Protocol.encode(prefix, content)
    :ok = :gen_tcp.send(socket, payload)
  end
end
