#!/usr/bin/env fish

if pgrep sigrok-cli
    echo "sigrok-cli is already running" > /dev/stderr
    exit 1
end

# TODO: Try to get the sigrok-cli to run until signal sent instead of setting duration?
# Unit: seconds
if set -q DURATION
    set sample_duration $DURATION
else
    set sample_duration 4
end

set script_dir (path dirname (status --current-filename))
set repo_root $script_dir/..

# debug_probe_vid=1fc9
# debug_probe_pid=0143

set sample_rate 50M
set logic_analyzer dreamsourcelab-dslogic
set channels "3=SWDIO,4=SWCLK,6=SWO"
set log_dir $repo_root/target/sniffing-logs
set out_dir $repo_root/target/sniffing-results

mkdir -p "$out_dir"
mkdir -p "$log_dir"

# echo "Start monitoring USB"
# # https://wiki.wireshark.org/CaptureSetup/USB
# lsusb=$(lsusb -d "$debug_probe_vid:$debug_probe_pid")
# lsusb_regex='^Bus 0+(?<bus>\d+) Device 0+(?<device>\d+): .*$'
# usb_bus=$(echo "$lsusb" | sd "$lsusb_regex" '$bus')
# usb_device=$(echo "$lsusb" | sd "$lsusb_regex" '$device')
# usbmon="usbmon$usb_bus"
# display_filter="usb.bus_id == $usb_bus and usb.device_address == $usb_device"
# # may need to
# #     modprobe usbmon
# #     setfacl -m u:$USER:r /dev/usbmon*
# tshark -i "$usbmon" -w "$out_dir/usb.pcapng" 1>$log_dir/tshark.log 2>$log_dir/tshark.err.log &

echo "Start monitoring SWD"
echo "Duration is $sample_duration"
echo "Redirecting logs to $log_dir"

sigrok-cli -d "$logic_analyzer" -C "$channels" --time "$sample_duration"s --config "samplerate=$sample_rate" 1>$log_dir/sigrok.log 2>$log_dir/sigrok.err.log -o "$out_dir/swd.sr" &

echo "Running the command.."
# TODO: how to setup timeout, hmm
timeout (math {$sample_duration}-0.5) $argv
echo "Command finished. Return code is $status"

# echo "Stop monitoring USB"
# killall --wait -SIGINT tshark
# echo "Post-processing captured USB"
# # tshark does what it is told, but complains about the end of the file because of the abrupt stop
# tshark -r "$out_dir/usb.pcapng" -Y "$display_filter" -w "$out_dir/usb.pcapng" 1>>$log_dir/tshark.log 2>>$log_dir/tshark.err.log || true

echo "Waiting for the background SWD monitoring to finish..."
wait (jobs -p)
echo "Post-processing captured SWD"
sigrok-cli -i "$out_dir/swd.sr" -P swd:swclk=SWCLK:swdio=SWDIO --protocol-decoder-samplenum > "$out_dir/swd.txt" 2>$log_dir/sigrok.err.log

echo "Done"
