#!/bin/bash
set -e

echo "=== top-collectd integration test ==="
echo ""

# Start collectd in the background
echo "Starting collectd..."
collectd -C /etc/collectd/collectd.conf -f &
COLLECTD_PID=$!

# Give collectd time to initialise and complete at least one read cycle.
# The plugin itself sleeps 500ms per read, plus the 10s collectd interval.
echo "Waiting 20s for collectd to complete a read cycle..."
sleep 20

# Check for CSV output
CSV_DIR="/var/lib/collectd/csv"
echo ""
echo "Checking for output in ${CSV_DIR}..."

if [ ! -d "${CSV_DIR}" ]; then
    echo "FAIL: CSV output directory does not exist"
    kill $COLLECTD_PID 2>/dev/null || true
    exit 1
fi

# Look for top plugin output: <hostname>/top-<process>/percent-cpu-*
TOP_FILES=$(find "${CSV_DIR}" -path "*/top-*/percent-cpu*" -type f 2>/dev/null)

if [ -z "${TOP_FILES}" ]; then
    echo "FAIL: No top plugin CSV files found"
    echo ""
    echo "Directory contents:"
    find "${CSV_DIR}" -type f 2>/dev/null || echo "  (empty)"
    echo ""
    echo "Collectd log (last 30 lines):"
    tail -30 /var/log/syslog 2>/dev/null || journalctl -u collectd --no-pager -n 30 2>/dev/null || echo "  (no logs available)"
    kill $COLLECTD_PID 2>/dev/null || true
    exit 1
fi

FILE_COUNT=$(echo "${TOP_FILES}" | wc -l)
echo "PASS: Found ${FILE_COUNT} top plugin CSV file(s):"
echo ""
echo "${TOP_FILES}" | while read -r f; do
    echo "  ${f}"
    echo "    $(head -1 "${f}")"
    echo "    $(tail -1 "${f}")"
done

echo ""
echo "=== Test passed ==="

# Clean up
kill $COLLECTD_PID 2>/dev/null || true
exit 0
