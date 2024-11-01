defmodule FubTest do
  use ExUnit.Case
  doctest Fub

  test "greets the world" do
    assert Fub.hello() == :world
  end
end
