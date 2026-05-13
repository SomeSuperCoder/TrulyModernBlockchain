defmodule Networking.Discovery do
  @moduledoc """
  Peer discovery GenServer.
  Will be implemented in Phase 4.
  """

  use GenServer

  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @impl true
  def init(_opts) do
    {:ok, %{peers: []}}
  end

  @impl true
  def handle_call(:get_peers, _from, state) do
    {:reply, state.peers, state}
  end
end
