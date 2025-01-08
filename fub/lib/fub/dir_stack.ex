defmodule Fub.DirStack do
  require Logger
  defstruct [:stack, :position]

  def new() do
    %__MODULE__{stack: [], position: 0}
  end

  def push(state, entry) do
    %{state | position: 0, stack: [entry | state.stack]}
  end

  def back(%__MODULE__{stack: []}) do
    :empty
  end

  def back(%__MODULE__{position: position} = state, entry) do
    state =
      if position == 0 do
        state
        |> push(entry)
        |> dedup()
      else
        state
      end

    position = min(position + 1, length(state.stack) - 1)

    {
      Enum.at(state.stack, position),
      %{state | position: position}
    }
  end

  def forward(%__MODULE__{stack: []}) do
    :empty
  end

  def forward(%__MODULE__{stack: stack, position: position} = state) do
    position = max(position - 1, 0)

    {
      Enum.at(stack, position),
      %{state | position: position}
    }
  end

  defp dedup(%__MODULE__{} = state) do
    %{state | stack: Enum.dedup(state.stack)}
  end
end
