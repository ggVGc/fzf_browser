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
    # "ctrl-x",
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

  @modes [:mixed, :files, :directories]
  # @recursion_levels [:no_recursion, :full, :relative_deepest_dir]
  @recursion_levels [:no_recursion, :full]

  def new(start_directory, start_query, full_recursive, mode \\ nil) do
    %__MODULE__{
      dir_stack: DirStack.new(),
      stored_query: start_query,
      current_directory: start_directory,
      deepest_dir: start_directory,
      flags: %{
        recursion_level_index: if(full_recursive, do: 1, else: 0),
        show_hidden: false,
        no_ignore: false,
        mode_index:
          case mode do
            "files" ->
              Enum.find_index(@modes, &(&1 == :files))

            "dir" ->
              Enum.find_index(@modes, &(&1 == :directories))

            _ ->
              if full_recursive do
                Enum.find_index(@modes, &(&1 == :files))
              else
                Enum.find_index(@modes, &(&1 == :mixed))
              end
          end
      }
    }
  end

  @impl true
  def get_preview_command(state) do
    path = "#{state.current_directory}/{}"
    # Assumes fzf_browser repo is in PATH
    # TODO: Rewrite in Elixir and respond with actual command
    "fzf-browser-preview.sh #{path}"
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
      | deepest_dir: state.current_directory,
        flags: %{
          state.flags
          | recursion_level_index:
              Integer.mod(state.flags.recursion_level_index + 1, length(@recursion_levels))
        }
    }
  end

  defp recursion_depth(path, deepest_dir) do
    relative = Path.relative_to(deepest_dir, path)

    case relative do
      "." ->
        0

      _ ->
        relative
        |> Path.split()
        |> length()
    end
  end

  defp build_prefixes(state) do
    flags = state.flags

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
        :no_recursion ->
          ["-"]
        :relative_deepest_dir ->
          [to_string(recursion_depth(state.current_directory, state.deepest_dir))]

        :full ->
          ["r"]
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
      case build_prefixes(state) do
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

      # Select directory
      # "/" ->
      #   full_path =
      #     [state.current_directory, selection]
      #     |> Path.join()
      #     |> Path.expand()
      #     |> Path.absname()

      #   {:exit, full_path}

      # "ctrl-x" ->
      #   if query == "." do
      #     {:exit, Path.absname(state.current_directory)}
      #   else
      #     # handle_selection(selection, query, state, &Path.absname/1)
      #     handle_selection(selection, query, state)
      #   end

      "ctrl-z" ->
        {:switch_source, Fub.Source.Recent.new()}

      key when key in @key_bindings ->
        state = %{state | stored_query: query}

        {:ok, state} = handle_continue_key(state, key, selection, query)
        {:continue, state}

      key ->
        Logger.error("Unhandled key: #{key}")
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

  defp handle_selection(selections, query, state) do
    full_paths =
      Enum.map(selections, fn selection ->
        [state.current_directory, selection]
        |> Path.join()
        |> Path.expand()
        |> Path.absname()
      end)

    case full_paths do
      [path] ->
        if File.dir?(path) do
          state = push_directory(state, path, query)

          {:continue, state}
        else
          # TODO: Only quote full_path if selection contains non-alphanumeric/period characters.
          {:exit, [path]}
        end

      paths ->
        {:exit, paths}
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
    old_deepest_dir = state.deepest_dir

    state =
      if dir_len(new_directory) > dir_len(state.deepest_dir) do
        %{state | deepest_dir: new_directory}
      else
        state
      end

    if File.dir?(new_directory) do
      dir_stack =
        DirStack.push(
          state.dir_stack,
          %{
            path: state.current_directory,
            query: current_query,
            deepest_dir: old_deepest_dir
          }
        )

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

  defp dir_down(state, selection, query) do
    [first | _] = Path.split(selection)
    new_path = Path.join([state.current_directory, first])

    if File.dir?(new_path) do
      state
      |> push_directory(new_path, query)
      |> put_query(query)
    else
      state
    end
  end

  defp dir_back(state, query) do
    case DirStack.back(state.dir_stack, %{
           path: state.current_directory,
           query: query,
           deepest_dir: state.deepest_dir
         }) do
      {%{path: new_directory, query: _query, deepest_dir: deepest_dir}, dir_stack} ->
        %{
          state
          | current_directory: new_directory,
            # stored_query: query,
            dir_stack: dir_stack,
            deepest_dir: deepest_dir
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
        ["--color=always", "--follow"],
        case recursion_level(flags) do
          :no_recursion ->
            ["--max-depth=1"]
          :relative_deepest_dir ->
            depth = recursion_depth(path, deepest_dir)
            ["--max-depth=#{depth + 1}"]

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

  defp put_query(state, query) do
    %{state | stored_query: query}
  end
end
