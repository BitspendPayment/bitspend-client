# Variables
RPCUSER = regtest
RPCPASSWORD = regtest
RPCPORT = 18443
DOCKER_COMPOSE_FILE = ./docker-compose.yml
TEST_DATA_FOLDER = ./testdata/bitcoin-wasm
BITCOIND_CONTAINER = bitcoind

# Default target
.PHONY: test
test: start setup run_test clean

# Pull and run bitcoind
.PHONY: start
start:
	@echo "Starting bitcoind..."
	@docker compose -f $(DOCKER_COMPOSE_FILE) up -d

# Run a command in a container
run-in-node:
	@docker exec $(BITCOIND_CONTAINER) /bin/bash -c "$(CMD)"

# Wait for a command to complete
wait-for-cmd:
	@until $(CMD) 2>&1; do \
		echo -n "."; \
		sleep 1; \
	done; \
	echo

# Wait for a command in a container
wait-for-node:
	$(MAKE) wait-for-cmd CMD="docker exec $(BITCOIND_CONTAINER) /bin/bash -c '$(CMD)'"


# Mine the first 101 blocks
.PHONY: mine-first-block
mine-first-block:
	@NEW_ADDRESS=$$(docker exec $(BITCOIND_CONTAINER) /bin/bash -c "bitcoin-cli -regtest -rpcwait -rpcport=$(RPCPORT) -rpcuser=$(RPCUSER) -rpcpassword=$(RPCPASSWORD) getnewaddress"); \
	echo $$NEW_ADDRESS; \
	$(MAKE) wait-for-node CMD="bitcoin-cli -regtest -rpcwait -rpcport=$(RPCPORT) -rpcuser=$(RPCUSER) -rpcpassword=$(RPCPASSWORD) generatetoaddress 101 $$NEW_ADDRESS"

# Setup wallet and initial mining
.PHONY: setup
setup:
	@echo "Setting up wallet and initial blocks..."
	$(MAKE) wait-for-node CMD="bitcoin-cli -regtest -rpcwait -rpcport=$(RPCPORT) -rpcuser=$(RPCUSER) -rpcpassword=$(RPCPASSWORD) getblockchaininfo"
	$(MAKE) wait-for-node CMD="bitcoin-cli -regtest -rpcwait -rpcport=$(RPCPORT) -rpcuser=$(RPCUSER) -rpcpassword=$(RPCPASSWORD) createwallet regtest > /dev/null"
	$(MAKE) mine-first-block


# Run the Rust tests
.PHONY: run_test
run_test:
	@echo "Running Rust tests..."
	@cargo run --package client-test --bin client-test || echo "Workflow failed, continuing to cleanup."

# Clean up
.PHONY: clean
clean:
	@echo "Stopping containers and cleaning up..."
	@docker compose -f $(DOCKER_COMPOSE_FILE) down
	rm -rf $(TEST_DATA_FOLDER) || true
