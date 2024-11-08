defmodule Fub.Source.Filesystem do
  @behaviour Fub.Source.Source

  require Logger
  alias Fub.DirStack

  defstruct [:current_directory, :dir_stack, :flags, :stored_query, :start_directory]

  @key_bindings [
    # Cycle mode
    "ctrl-l",
    # Select full path
    "ctrl-x",
    # Go into directory, or open file
    # "right",
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

  @modes [:mixed, :files, :directories]
  @recursion_levels [:none, :single, :full]

  def new(start_directory) do
    %__MODULE__{
      dir_stack: DirStack.new(),
      stored_query: "",
      start_directory: start_directory,
      current_directory: start_directory,
      flags: %{
        sort: false,
        recursion_level_index: 0,
        show_hidden: false,
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
        :none -> []
        :single -> ["1"]
        :full -> ["-"]
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
  def get_content(%__MODULE__{current_directory: path, flags: flags}) do
    list_dir(path, flags)
  end

  @impl true
  def handle_result(state, selection, query, key) do
    case key do
      "ctrl-z" ->
        {:switch_source, Fub.Source.Recent.new()}

      "ctrl-x" ->
        if query == "." do
          {:exit, state.current_directory}
        else
          handle_selection(selection, query, state, & &1)
        end

      "" ->
        if query == "." do
          {:exit, Path.relative_to(state.current_directory, state.start_directory)}
        else
          handle_selection(
            selection,
            query,
            state,
            &Path.relative_to(&1, state.start_directory)
          )
        end

      key when key in @key_bindings ->
        state = %{state | stored_query: query}

        {:ok, state} = handle_key(key, state)
        {:continue, state}

      tag ->
        Logger.error("Unhandled message tag: #{tag}")
        {:continue, state}
    end
  end

  defp handle_key(key, state) do
    state =
      case key do
        "ctrl-l" ->
          cycle_mode(state)

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
          cycle_rec_level(state)

        _ ->
          Logger.error("Unhandled key: #{key}")
          state
      end

    {:ok, state}
  end

  defp handle_selection(selection, query, state, path_transformer) do
    full_path = Path.join(state.current_directory, selection)

    if File.dir?(full_path) do
      state =
        %{
          push_directory(state, full_path, query)
          | stored_query: ""
        }

      {:continue, state}
    else
      result = path_transformer.(full_path)

      # Only quote result if selection contains non-alphanumeric/period characters.
      {:exit, result}
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

  defp list_dir(path, flags) do
    fd_args =
      List.flatten([
        ["--color=always"],
        case recursion_level(flags) do
          :none -> ["--max-depth=1"]
          :single -> ["--max-depth=2"]
          :full -> []
        end,
        if(flags.show_hidden, do: ["-H"], else: []),
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

    if flags.sort do
      Enum.sort(content)
    else
      content
    end
  end
end
