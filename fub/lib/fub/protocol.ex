# TODO: Replace with HTTP, or something else that isn't homegrown...

defmodule Fub.Protocol do
  def encode(:begin_entries, content) do
    {:ok, "e" <> content <> "\n"}
  end

  def encode(:raw, content) do
    {:ok, content}
  end

  def encode(:end_entries, nil) do
    {:ok, "\n"}
  end

  def encode(:exit, output) do
    {:ok, "x" <> output <> "\n"}
  end

  def encode(:end_of_content, nil) do
    {:ok, "z\n"}
  end

  def encode(:open_finder, payload) do
    {:ok, "o" <> Jason.encode!(payload) <> "\n"}
  end
end
