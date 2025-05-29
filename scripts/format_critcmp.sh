#!/bin/bash
set -euo pipefail

echo '| Group | main | pr |'
echo '|-------|------|----|'

while IFS= read -r line; do
  # Skip empty lines and headers
  [[ -z "$line" || "$line" =~ ^group || "$line" =~ ^[-[:space:]]+$ ]] && continue

  # Split by 2+ spaces into group, main, pr
  IFS='|' read -r group rest <<< "$(echo "$line" | sed -E 's/  +/|/g')"
  IFS='|' read -r main pr <<< "$rest"

  # Trim whitespace
  group="$(echo "$group" | xargs)"
  main="$(echo "$main" | xargs)"
  pr="$(echo "$pr" | xargs)"

  echo "| \`$group\` | \`$main\` | \`$pr\` |"
done
