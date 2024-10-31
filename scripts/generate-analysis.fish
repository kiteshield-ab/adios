#!/usr/bin/env fish

if test (count $argv) != 1
    echo "Provide a name for the analysis run" > /dev/stderr
    exit 1
end

set analysis_name $argv[1]

set script_dir (path dirname (status --current-filename))
set repo_root $script_dir/..

set svds --svd $repo_root/svds/MIMXRT1189_cm33.svd --svd $repo_root/svds/MIMXRT1189_cm33-SecureExt.svd --svd $repo_root/svds/CortexM33.svd

set output $repo_root/target/analysis-results
set input $repo_root/target/sniffing-results/swd.txt

set path1 $output/$analysis_name.swd.adios
set path2 $output/$analysis_name.with-ts.swd.adios

mkdir -p $output && \
cargo run --release -- \
--mode sigrok-swd -m -M --dp --ap \
--input $input \
$svds \
> $path1 && \
echo "File $path1 generated" && \
\
cargo run --release -- \
--mode sigrok-swd  -M --dp --ap --ts \
--input $input \
$svds \
> $path2 && \
echo "File $path2 generated"
