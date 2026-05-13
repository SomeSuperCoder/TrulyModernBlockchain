defmodule Networking.Application do
  @moduledoc """
  OTP Application for the blockchain networking layer.
  """

  use Application

  @impl true
  def start(_type, _args) do
    children = [
      {Networking.Discovery, []},
      {Networking.Gossip, []},
      {Networking.ConnectionManager, []}
    ]

    opts = [strategy: :one_for_one, name: Networking.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
