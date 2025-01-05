defmodule Fub.Source.Recent do
  @behaviour Fub.Source.Source
  defstruct []

  def new() do
    %__MODULE__{}
  end

  @impl true
  def get_launch_info(%__MODULE__{}) do
    %{query_prefix: "", key_bindings: ["ctrl-z"], query: ""}
  end

  @impl true
  def get_content(%__MODULE__{}) do
    list_recent_locations()
  end

  @impl true
  def handle_result(%__MODULE__{}, [selection | _], _query, key) do
    case key do
      "ctrl-z" ->
        {:switch_source, :previous}

      "" ->
        {:switch_source, Fub.Source.Filesystem.new(selection, "", false)}
    end
  end

  defp list_recent_locations() do
    %Porcelain.Process{out: content} = Porcelain.spawn("fasd", ["-ld"], out: :stream)
    content
  end
end
