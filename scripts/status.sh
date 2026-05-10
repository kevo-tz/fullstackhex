#!/bin/bash
# status.sh — Show which development services are running and on which ports.
# Reads PID files from the PID_DIR (config.sh) written by make dev.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

API_BASE="${API_BASE:-http://localhost:8001}"

# Header
printf "%-16s %-8s %-8s %-12s %s\n" "SERVICE" "PID" "PORT" "HEALTH" "UPTIME"
printf "%s\n" "--------------------------------------------------------------"

check_pid() {
    local name
    name="$1" pidfile="$2" port="${3:--}"
    if [ -f "$pidfile" ]; then
        local pid
        pid=$(cat "$pidfile" 2>/dev/null || echo "")
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            local pid_display
            pid_display="$pid"
            local uptime
            uptime=$(ps -o etime= -p "$pid" 2>/dev/null | tr -d ' ' || echo "-")
        else
            local pid_display
            pid_display="-"
            local uptime
            uptime="-"
        fi
    else
        local pid_display
        pid_display="-"
        local uptime
        uptime="-"
    fi

    local health
    health="-"
    if [ -n "$port" ] && [ "$port" != "-" ]; then
        health=$(curl -sk --max-time 2 "$API_BASE$port" 2>/dev/null \
            | grep -o '"status":"[^"]*"' | head -1 \
            | sed 's/"status":"//;s/"//' 2>/dev/null || echo "-")
    fi

    printf "%-16s %-8s %-8s %-12s %s\n" "$name" "${pid_display:-"-"}" "$port" "${health:-"-"}" "${uptime:-"-"}"
}

check_pid "Rust API"     "$PID_DIR/backend.pid"   "/health"
check_pid "Frontend"     "$PID_DIR/frontend.pid"  "-"
check_pid "Python"       "$PID_DIR/python.pid"    "/health/python"
check_pid "PostgreSQL"   ""                       "/health/db"
check_pid "Redis"        ""                       "/health/redis"

echo ""
echo "PID directory: $PID_DIR"
echo "Backend log:   $PID_DIR/backend.log"
