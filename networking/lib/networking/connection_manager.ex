defmodule Networking.ConnectionManager do
  @moduledoc """
  Connection management supervisor.
  Will be implemented in Phase 4.
  """

  use GenServer

  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @impl true
  def init(_opts) do
    {:ok, %{connections: %{}}}
  end

  @impl true
  def handle_call({:connect, _peer_id}, _from, state) do
    # TODO: Implement connection management
    {:reply, :ok, state}
  end
end
