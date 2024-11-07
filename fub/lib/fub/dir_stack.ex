defmodule Fub.DirStack do
  defstruct [:stack, :position]

  def new() do
    %__MODULE__{stack: [], position: 0}
  end

  def push(mod, path, query) do
    %{mod | position: 0, stack: [%{path: path, query: query} | mod.stack]}
  end

  def back(%__MODULE__{stack: []}) do
    :empty
  end

  def back(%__MODULE__{stack: stack, position: position} = mod) do
    position = min(position + 1, length(stack) - 1)

    {
      Enum.at(stack, position),
      %{mod | position: position}
    }
  end

  def forward(%__MODULE__{stack: []}) do
    :empty
  end

  def forward(%__MODULE__{stack: stack, position: position} = mod) do
    position = max(position - 1, 0)

    {
      Enum.at(stack, position),
      %{mod | position: position}
    }
  end
end
