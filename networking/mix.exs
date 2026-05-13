defmodule Networking.MixProject do
  use Mix.Project

  def project do
    [
      app: :networking,
      version: "0.1.0",
      elixir: "~> 1.15",
      start_permanent: Mix.env() == :prod,
      deps: deps()
    ]
  end

  def application do
    [
      extra_applications: [:logger],
      mod: {Networking.Application, []}
    ]
  end

  defp deps do
    [
      {:protobuf, "~> 0.12"},
      {:jason, "~> 1.4"},
      {:ranch, "~> 2.1"}
    ]
  end
end
