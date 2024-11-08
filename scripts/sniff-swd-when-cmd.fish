#!/usr/bin/env fish

if pgrep sigrok-cli
    echo "sigrok-cli is already running" > /dev/stderr
    exit 1
end

# Unit: seconds
if set -q TIMEOUT
    set timeout $TIMEOUT
else
    set timeout 5
end

set script_dir (path dirname (status --current-filename))
set repo_root $script_dir/..

# debug_probe_vid=1fc9
# debug_probe_pid=0143

set sample_rate 1M
set logic_analyzer dreamsourcelab-dslogic
set channels "3=SWDIO,4=SWCLK,6=SWO"
set out_dir $repo_root/target/sniffing-results
set log_dir $out_dir/logs

mkdir -p "$out_dir"
mkdir -p "$log_dir"

echo "Start monitoring SWD"
echo "Redirecting logs to $log_dir"

set sigrok_timeout (math {$timeout} + 3)s
echo "Sigrok timeout set to \"$sigrok_timeout\""
sigrok-cli -d "$logic_analyzer" -C "$channels" --time $sigrok_timeout --config "samplerate=$sample_rate" 1>$log_dir/sigrok.log 2>$log_dir/sigrok.err.log -o "$out_dir/swd.sr" &

# Give sigrok time to warm up I guess
sleep 2

echo "Running the command.."
timeout $timeout $argv &| tee $log_dir/command.log
echo "Command finished/timed out. Return code is $status"

echo "Waiting for the background SWD monitoring to finish..."
wait (jobs -p)
echo "Post-processing captured SWD"
sigrok-cli -i "$out_dir/swd.sr" -P swd:swclk=SWCLK:swdio=SWDIO --protocol-decoder-samplenum > "$out_dir/swd.txt" 2>$log_dir/sigrok.err.log

echo "Done"
