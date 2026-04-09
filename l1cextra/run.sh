#!/bin/bash

# Config
BASE_PORT=59001
SERVER_COUNT=6
BINARY_PATH="./target/release/l1cextra"
START_DELAY=1.5

if [ ! -f "$BINARY_PATH" ]; then
  echo "Binary not found at $BINARY_PATH"
  exit 1
fi

echo "Starting $SERVER_COUNT servers..."

PIDS=()

for ((i=1; i<=SERVER_COUNT; i++)); do
  $BINARY_PATH $i $BASE_PORT $SERVER_COUNT &
  PID=$!
  PIDS+=($PID)
  echo "Started server $i with PID $PID"

  sleep $START_DELAY
done

cleanup() {
  echo ""
  echo "Stopping all servers..."
  for PID in "${PIDS[@]}"; do
    kill $PID 2>/dev/null
  done
  wait
  echo "All servers stopped."
  exit 0
}

trap cleanup SIGINT SIGTERM

wait
