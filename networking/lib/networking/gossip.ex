defmodule Networking.Gossip do
  @moduledoc """
  Gossip protocol GenServer.
  Will be implemented in Phase 4.
  """

  use GenServer

  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @impl true
  def init(_opts) do
    {:ok, %{messages: []}}
  end

  @impl true
  def handle_cast({:broadcast, message}, state) do
    # TODO: Implement gossip broadcasting
    {:noreply, Map.update(state, :messages, [message], &[message | &1])}
  end
end
