defmodule Fub.MixProject do
  use Mix.Project

  def project do
    [
      app: :fub,
      version: "0.1.0",
      elixir: "~> 1.12",
      start_permanent: Mix.env() == :prod,
      deps: deps(),
      aliases: [run: ["run --no-halt"]]
    ]
  end

  def application do
    [
      extra_applications: [:logger],
      mod: {Fub.Application, []}
    ]
  end

  defp deps do
    [
      {:exsync, "~> 0.4", only: :dev},
      {:jason, "~> 1.4.4"},
      {:porcelain, "~> 2.0.3"}
    ]
  end
end
