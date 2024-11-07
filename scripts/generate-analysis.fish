#!/usr/bin/env fish

argparse --max-args 1 --min-args 1 'O/override' -- $argv || exit 1

set analysis_name $argv[1]

set script_dir (path dirname (status --current-filename))
set repo_root $script_dir/..
set svds --svd $repo_root/svds/MIMXRT1189_cm33.svd --svd $repo_root/svds/MIMXRT1189_cm33-SecureExt.svd --svd $repo_root/svds/CortexM33.svd
set src $repo_root/target/sniffing-results
set work_dir $repo_root/target/analysis-results/$analysis_name
set input_dir $work_dir/input
set input $input_dir/swd.txt
set output_dir $work_dir/output
set out_file_swd $output_dir/analysis.swd.adios
set out_file_swd_with_ts $output_dir/analysis.with-ts.swd.adios

mkdir -p $output_dir || exit 1

if not test -f "$input"; or test -n "$_flag_override"
    rm -rf "$input_dir"
    cp -r $src $input_dir || exit 1
    echo "copied $src to $input_dir"
else
    echo "reusing $input"
end

cargo run --release -- \
--mode sigrok-swd -m -M --dp --ap \
--input $input \
$svds \
> $out_file_swd && \
echo "file $out_file_swd generated" && \
\
cargo run --release -- \
--mode sigrok-swd -m -M --dp --ap --ts \
--input $input \
$svds \
> $out_file_swd_with_ts && \
echo "file $out_file_swd_with_ts generated"
