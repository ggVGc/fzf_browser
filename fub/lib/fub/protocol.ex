# TODO: Replace with HTTP, or something else that isn't homegrown...

defmodule Fub.Protocol do
  def encode(:entry, entry) do
    {:ok, "e" <> entry <> "\n"}
  end

  def encode(:exit, output) do
    {:ok, "x" <> output <> "\n"}
  end

  def encode(:wait_for_response, nil) do
    {:ok, "w\n"}
  end

  def encode(:open_finder, payload) do
    {:ok, "o" <> Jason.encode!(payload) <> "\n"}
  end
end
