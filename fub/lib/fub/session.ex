defmodule Fub.Session do
  require Logger
  alias Fub.DirStack

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
      stored_query: "",
      dir_stack: DirStack.new(),
      flags: %{
        sort: false,
        recursion_level: 0,
        show_hidden: false,
        # mode: :files, :directories, :mixed
        mode: :mixed
      },
      cache: %{}
    }

    loop(client_socket, state)
  end

  defp open_finder(socket, query, current_directory) do
    respond(socket, :open_finder, %{
      query: query,
      key_bindings: @key_bindings,
      with_ansi_colors: true,
      current_directory: current_directory
    })
  end

  defp list_dir(socket, path, flags) do
    fd_args =
      List.flatten([
        ["--color=always"],
        case flags.recursion_level do
          0 -> ["--max-depth=1"]
          1 -> ["--max-depth=2"]
          2 -> []
        end,
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

    File.cd!(path)
    %Porcelain.Process{out: content} = Porcelain.spawn("fd", fd_args, out: :stream)

    content =
      if flags.sort do
        Enum.sort(content)
      else
        content
      end

    respond(socket, :begin_entries)

    # TODO: Run asynchronously stop streaming if any command 
    # is received from client.
    Enum.each(content, fn chunk ->
      respond(socket, :raw, chunk)
    end)

    respond(socket, :end_entries)
    respond(socket, :end_of_content)
    :ok
  end

  defp list_current_dir(socket, state) do
    open_finder(socket, state.stored_query, state.current_directory)
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
        query = Map.fetch!(message, "query")

        case Map.fetch!(message, "key") do
          key when key in @key_bindings ->
            state = %{state | stored_query: query}
            {:ok, state} = handle_key(key, state)
            list_current_dir(socket, state)

          "" ->
            selection = Map.fetch!(message, "selection")
            handle_selection(socket, selection, query, state)

          tag ->
            Logger.error("Unhandled message tag: #{tag}")
            list_current_dir(socket, state)
        end
    end
  end

  defp handle_selection(socket, selection, query, state) do
    full_path = Path.join(state.current_directory, selection)

    if File.dir?(full_path) do
      state =
        %{
          push_directory(state, full_path, query)
          | stored_query: ""
        }

      list_current_dir(socket, state)
    else
      result = Path.relative_to(full_path, state.start_directory)
      # Only quote result if selection contains non-alphanumeric/period characters.
      respond(socket, :exit, "'#{result}'")
      {:ok, state}
    end
  end

  defp toggle_flag(state, name) do
    %{state | flags: %{state.flags | name => not Map.fetch!(state.flags, name)}}
  end

  defp push_directory(state, new_directory, current_query) do
    %{
      state
      | dir_stack: DirStack.push(state.dir_stack, state.current_directory, current_query),
        current_directory: new_directory
    }
  end

  defp dir_back(state) do
    case DirStack.back(state.dir_stack) do
      :empty ->
        state

      {%{path: new_directory, query: query}, dir_stack} ->
        %{
          state
          | current_directory: new_directory,
            stored_query: query,
            dir_stack: dir_stack
        }
    end
  end

  defp dir_forward(state) do
    case DirStack.forward(state.dir_stack) do
      :empty ->
        state

      {%{path: new_directory, query: query}, dir_stack} ->
        %{
          state
          | current_directory: new_directory,
            stored_query: query,
            dir_stack: dir_stack
        }
    end
  end

  defp handle_key(key, state) do
    state =
      case key do
        "ctrl-h" ->
          state = push_directory(state, Path.expand("~"), state.stored_query)
          %{state | stored_query: ""}

        "ctrl-o" ->
          dir_back(state)

        "ctrl-u" ->
          dir_forward(state)

        sort_toggle when sort_toggle in ["ctrl-s", "ctrl-y"] ->
          toggle_flag(state, :sort)

        "ctrl-a" ->
          toggle_flag(state, :show_hidden)

        "\\" ->
          %{
            state
            | flags: %{
                state.flags
                | recursion_level: Integer.mod(state.flags.recursion_level + 1, 3)
              }
          }

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
