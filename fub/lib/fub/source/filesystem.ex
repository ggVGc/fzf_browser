defmodule Fub.Source.Filesystem do
  @behaviour Fub.Source.Source

  require Logger
  alias Fub.DirStack

  defstruct [
    :current_directory,
    :dir_stack,
    :flags,
    :stored_query,
    :deepest_dir
  ]

  @key_bindings [
    # Cycle mode
    "ctrl-f",
    "]",
    # Select full path
    "ctrl-x",
    # Select directory
    "/",
    # Go up one directory
    "left",
    "ctrl-h",
    "`",
    # Down one directory
    "right",
    "ctrl-l",
    # Dir stack back
    "ctrl-o",
    # Dir stack forward
    "ctrl-u",
    # Go to home directory
    "ctrl-d",
    # Launch directory jumper (currently fasd -ld)
    "ctrl-z",
    # Go to directory of selected file
    "ctrl-g",
    # Toggle hidden files
    "ctrl-a",
    # Toggle no-ignore
    "ctrl-y",
    # Recursive
    "\\",
    # Set deepest dir to current
    "ctrl-t"
  ]

  @modes [:mixed, :directories, :files]
  @recursion_levels [:relative_deepest_dir, :full]

  def new(start_directory, start_query, full_recursive) do
    %__MODULE__{
      dir_stack: DirStack.new(),
      stored_query: start_query,
      current_directory: start_directory,
      deepest_dir: start_directory,
      flags: %{
        recursion_level_index:
          if full_recursive do
            1
          else
            0
          end,
        show_hidden: false,
        no_ignore: false,
        mode_index: 0
      }
    }
  end

  defp mode(flags) do
    Enum.at(@modes, flags.mode_index)
  end

  defp recursion_level(flags) do
    Enum.at(@recursion_levels, flags.recursion_level_index)
  end

  defp cycle_mode(state) do
    %{
      state
      | flags: %{
          state.flags
          | mode_index: Integer.mod(state.flags.mode_index + 1, length(@modes))
        }
    }
  end

  defp cycle_rec_level(state) do
    %{
      state
      | flags: %{
          state.flags
          | recursion_level_index:
              Integer.mod(state.flags.recursion_level_index + 1, length(@recursion_levels))
        }
    }
  end

  defp build_prefixes(flags) do
    List.flatten([
      case mode(flags) do
        :mixed ->
          ["M"]

        :files ->
          ["F"]

        :directories ->
          ["D"]
      end,
      case recursion_level(flags) do
        :relative_deepest_dir -> ["-"]
        :full -> ["r"]
      end,
      if flags.no_ignore do
        ["I"]
      else
        []
      end,
      if flags.show_hidden do
        ["H"]
      else
        []
      end
    ])
  end

  @impl true
  def get_launch_info(%__MODULE__{} = state) do
    prefix =
      case build_prefixes(state.flags) do
        [] ->
          ""

        prefixes ->
          "[#{Enum.join(prefixes, ",")}]"
      end

    %{
      query_prefix: prefix <> " #{state.current_directory}",
      key_bindings: @key_bindings,
      query: state.stored_query
    }
  end

  @impl true
  def get_content(%__MODULE__{} = state) do
    list_dir(state.current_directory, state.deepest_dir, state.flags)
  end

  @impl true
  def handle_result(state, selection, query, key) do
    case key do
      "" ->
        if query == "." do
          # {:exit, Path.relative_to(state.current_directory, state.launch_directory)}
          {:exit, state.current_directory}
        else
          handle_selection(selection, query, state)
        end

      "/" ->
        full_path =
          [state.current_directory, selection]
          |> Path.join()
          |> Path.expand()
          |> Path.absname()

        {:exit, full_path}

      "ctrl-x" ->
        if query == "." do
          {:exit, Path.absname(state.current_directory)}
        else
          # handle_selection(selection, query, state, &Path.absname/1)
          handle_selection(selection, query, state)
        end

      "ctrl-z" ->
        {:switch_source, Fub.Source.Recent.new()}

      key when key in @key_bindings ->
        state = %{state | stored_query: query}

        {:ok, state} = handle_continue_key(state, key, selection, query)
        {:continue, state}

      tag ->
        Logger.error("Unhandled message tag: #{tag}")
        {:continue, state}
    end
  end

  defp handle_continue_key(state, key, selection, current_query) do
    state =
      case key do
        cycle_key when cycle_key in ["ctrl-f", "]"] ->
          cycle_mode(state)

        "ctrl-t" ->
          set_deepest_dir_to_current(state)

        "ctrl-d" ->
          goto_home(state, current_query)

        "ctrl-g" ->
          enter_path_directory(state, selection, current_query)

        "ctrl-o" ->
          dir_back(state, current_query)

        "ctrl-u" ->
          dir_forward(state)

        "ctrl-a" ->
          toggle_flag(state, :show_hidden)

        "ctrl-y" ->
          toggle_flag(state, :no_ignore)

        dir_up_key when dir_up_key in ["left", "ctrl-h", "`"] ->
          dir_up(state, current_query)

        dir_down_key when dir_down_key in ["right", "ctrl-l"] ->
          dir_down(state, selection, current_query)

        "\\" ->
          cycle_rec_level(state)

        _ ->
          Logger.error("Unhandled key: #{key}")
          state
      end

    {:ok, state}
  end

  defp set_deepest_dir_to_current(state) do
    %{state | deepest_dir: state.current_directory}
  end

  defp enter_path_directory(state, selection, current_query) do
    directory = Path.join([state.current_directory, selection])

    directory =
      if File.dir?(directory) do
        directory
      else
        Path.dirname(directory)
      end

    push_directory(state, directory, current_query)
  end

  defp handle_selection(selection, query, state) do
    full_path =
      [state.current_directory, selection]
      |> Path.join()
      |> Path.expand()
      |> Path.absname()

    if File.dir?(full_path) do
      state = push_directory(state, full_path, query)

      {:continue, state}
    else
      # Only quote full_path if selection contains non-alphanumeric/period characters.
      {:exit, full_path}
    end
  end

  defp toggle_flag(state, name) do
    %{state | flags: %{state.flags | name => not Map.fetch!(state.flags, name)}}
  end

  defp goto_home(state, current_query) do
    home = Path.expand("~")
    state = push_directory(state, home, current_query)
    %{state | stored_query: "", deepest_dir: home}
  end

  defp dir_len(path) do
    path |> Path.split() |> length()
  end

  defp push_directory(state, new_directory, current_query) do
    new_directory = Path.expand(new_directory)

    state =
      if dir_len(new_directory) > dir_len(state.deepest_dir) do
        %{state | deepest_dir: new_directory}
      else
        state
      end

    if File.dir?(new_directory) do
      dir_stack = DirStack.push(state.dir_stack, state.current_directory, current_query)

      %{
        state
        | dir_stack: dir_stack,
          current_directory: new_directory,
          stored_query: ""
      }
    else
      state
    end
  end

  defp dir_up(state, query) do
    new_directory = Path.join([state.current_directory, ".."])

    state
    |> push_directory(new_directory, query)
    |> put_query(query)
  end

  defp put_query(state, query) do
    %{state | stored_query: query}
  end

  defp dir_down(state, selection, query) do
    [first | _] = Path.split(selection)
    new_path = Path.join([state.current_directory, first])

    if File.dir?(new_path) do
      push_directory(state, new_path, query)
    else
      state
    end
  end

  defp dir_back(state, query) do
    case DirStack.back(state.dir_stack, state.current_directory, query) do
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

  defp list_dir(path, deepest_dir, flags) do
    fd_args =
      List.flatten([
        ["--color=always"],
        case recursion_level(flags) do
          :relative_deepest_dir ->
            relative = Path.relative_to(deepest_dir, path)

            count =
              case relative do
                "." ->
                  0

                _ ->
                  relative
                  |> Path.split()
                  |> length()
              end

            ["--max-depth=#{count + 1}"]

          :full ->
            []
        end,
        if(flags.show_hidden, do: ["-H"], else: []),
        if(flags.no_ignore, do: ["-I"], else: []),
        case mode(flags) do
          :directories ->
            ["--type", "d"]

          :files ->
            ["--type", "f"]

          :mixed ->
            []
        end
      ])

    Logger.debug("fd_args: #{inspect(fd_args)}")
    %Porcelain.Process{out: content} = Porcelain.spawn("fd", fd_args, out: :stream, dir: path)
    content
  end
end
