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
    # Recursive
    "\\"
  ]

  def new(client_socket) do
    state = %{
      start_directory: nil,
      current_directory: nil,
      previous_directory: nil,
      stored_query: "",
      dir_stack: [],
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

    Logger.debug("fd_args: #{inspect(fd_args)}")

    # TODO: Stream instead of buffering whole output. Maybe use porcelain.
    {content, 0} = System.cmd("fd", fd_args, cd: path)

    content = String.split(content, "\n")

    content =
      if flags.sort do
        Enum.sort(content)
      else
        content
      end

    respond(socket, :begin_entries)

    Enum.each(content, fn entry ->
      if entry != "" do
        respond(socket, :entry, entry)
      end
    end)

    respond(socket, :end_entries)
    respond(socket, :end_of_content)
    :ok
  end

  defp list_current_dir(socket, state) do
    open_finder(socket, state.stored_query)
    list_dir(socket, state.current_directory, state.flags)
    {:ok, state}
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

    state = %{state | start_directory: start_directory, current_directory: start_directory}
    list_current_dir(socket, state)
  end

  defp handle_message(socket, %{"tag" => "result"} = message, state) do
    case Map.fetch!(message, "code") do
      code when code in [0, 1] ->
        case Map.fetch!(message, "key") do
          key when key in @key_bindings ->
            query = Map.fetch!(message, "query")
            {:ok, state} = handle_key(key, state)
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
      state = push_directory(state, full_path)
      list_current_dir(socket, state)
    else
      result = Path.relative_to(full_path, state.start_directory)
      # Only quote result if selection contains non-alphanumeric/period characters.
      respond(socket, :exit, "'#{result}'")
      {:ok, state}
    end
  end

  defp toggle_flag(state, name) do
    %{state | flags: %{state.flags | show_hidden: not Map.fetch!(state.flags, name)}}
  end

  defp push_directory(state, new_directory) do
    %{
      state
      | dir_stack: [state.current_directory | state.dir_stack],
        previous_directory: state.current_directory,
        current_directory: new_directory
    }
    |> IO.inspect(label: "state")
  end

  defp pop_directory(state) do
    case state.dir_stack do
      [] ->
        state

      [new_directory | rest] ->
        %{
          state
          | dir_stack: rest,
            current_directory: new_directory,
            previous_directory: state.current_directory
        }
    end
  end

  defp push_previous_dir(state) do
    if is_nil(state.previous_directory) do
      state
    else
      push_directory(state, state.previous_directory)
    end
  end

  defp handle_key(key, state) do
    state =
      case key do
        "ctrl-h" ->
          push_directory(state, Path.expand("~"))

        "ctrl-o" ->
          pop_directory(state)

        "ctrl-u" ->
          push_previous_dir(state)

        sort_toggle when sort_toggle in ["ctrl-s", "ctrl-y"] ->
          toggle_flag(state, :sort)

        "ctrl-a" ->
          toggle_flag(state, :show_hidden)

        "\\" ->
          toggle_flag(state, :recursive)

        _ ->
          Logger.error("Unhandled key: #{key}")
          state
      end

    {:ok, state}
  end

  defp respond(socket, prefix, content \\ nil) do
    {:ok, payload} = Fub.Protocol.encode(prefix, content)
    :ok = :gen_tcp.send(socket, payload)
  end
end
