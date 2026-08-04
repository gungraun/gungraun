#!/usr/bin/env perl

# cspell: ignore libc

# A helper script to turn callgrind_annotate output into stack format as it is
# required by inferno and returned by our FlamegraphParser ->
# FlamegraphMap::to_stack_format.
#
# The output of this script is used for testing purposes to compare the stacks of the
# FlamegraphMap with the output of callgrind_annotate. This script is currently
# limited to create stacks from Ir events.

use strict;
use warnings;

my $usage = <<END
usage: callgrind_annotate_to_stacks [options]

  This script expects callgrind_annotate output with --inclusive=yes
  and no tree or other options on stdin. The converted stacks are
  printed to stdout.

  options for the user, with defaults in [ ], are:
    -h --help                         show this message
    --sentinel=<string>               use this sentinel ['main']
    --insert='<pos> <string> <cost>'  insert <string> at <pos> with <cost>
    --add-missing-ob=<string>         fill missing object files with <string>
    --modify=<string> <cost>          modify a string to new <cost>
    --replace=<string>==>><string>    replace occurrences of first <string> with second <string>
END
  ;

my $sentinel = "main";
my @insert;
my @replace;
my $missing_ob;
my @modify;

for my $arg (@ARGV) {
    if ( $arg =~ /^-/ ) {
        if ( $arg =~ /^--sentinel=(.*)$/ ) {
            $sentinel = "$1";
        }
        elsif ( $arg =~ /^--insert=(\d+) (.*) (\d+)$/ ) {
            my @rec = ( $1, $2, $3 );
            push( @insert, \@rec );
        }
        elsif ( $arg =~ /^--replace=(.*)==>>(.*)$/ ) {
            my @rec = ( $1, $2 );
            push( @replace, \@rec );
        }
        elsif ( $arg =~ /^--modify=(.*) (\d+)$/ ) {
            my @rec = ( $1, $2 );
            push( @modify, \@rec );
        }
        elsif ( $arg =~ /^--add-missing-ob=(.*)$/ ) {
            $missing_ob = "$1";
        }
        else {
            die($usage);
        }
    }
}

my $pwd = `pwd`;
chomp $pwd;
$pwd .= '/';

my @sources = ();
my @stacks;
my $sentinel_source;
my $sentinel_count;

# Here some example lines we need to be able to parse into stack format.
#
# 3,473 (100.0%)   889 (100.0%)   559 (100.0%) 143 (100.0%) 30 (100.0%)  7 (100.0%) 116 (100.0%) 4 (100.0%) 4 (100.0%)  ???:0x000000000001b530 [/usr/lib/ld-linux-x86-64.so.2]
# 3,473 (100.0%)   889 (100.0%)   559 (100.0%) 143 (100.0%) 30 (100.0%)  7 (100.0%) 116 (100.0%) 4 (100.0%) 4 (100.0%)  ???:0x0000000000027c50 [/usr/lib/libc.so.6]
# 3,473 (100.0%)   889 (100.0%)   559 (100.0%) 143 (100.0%) 30 (100.0%)  7 (100.0%) 116 (100.0%) 4 (100.0%) 4 (100.0%)  ???:gungraun_tests_exit::main [/home/lenny/workspace/programming/gungraun/target/release/gungraun-tests-exit]
# 3,473 (100.0%)   889 (100.0%)   559 (100.0%) 143 (100.0%) 30 (100.0%)  7 (100.0%) 116 (100.0%) 4 (100.0%) 4 (100.0%)  ???:main [/home/lenny/workspace/programming/gungraun/target/release/gungraun-tests-exit]
# 3,473 (100.0%)   889 (100.0%)   559 (100.0%) 143 (100.0%) 30 (100.0%)  7 (100.0%) 116 (100.0%) 4 (100.0%) 4 (100.0%)  ???:std::rt::lang_start::{{closure}} [/home/lenny/workspace/programming/gungraun/target/release/gungraun-tests-exi>
# 3,473 (100.0%)   889 (100.0%)   559 (100.0%) 143 (100.0%) 30 (100.0%)  7 (100.0%) 116 (100.0%) 4 (100.0%) 4 (100.0%)  ???:std::sys_common::backtrace::__rust_begin_short_backtrace [/home/lenny/workspace/programming/gungraun/target/rel>
# 2,173 (62.57%)   609 (68.50%)   399 (71.38%) 117 (81.82%) 25 (83.33%)  4 (57.14%)  90 (77.59%) 4 (100.0%) 1 (25.00%)  /rustc/7737e0b5c4103216d6fd8cf941b7ab9bdbaace7c//library/std/src/process.rs:std::process::exit [/home/lenny/workspace/pr>
# 1,938 (55.80%)   549 (61.75%)   349 (62.43%)  91 (63.64%) 22 (73.33%)  4 (57.14%)  74 (63.79%) 3 (75.00%) 1 (25.00%)  /rustc/7737e0b5c4103216d6fd8cf941b7ab9bdbaace7c//library/std/src/sys/unix/os.rs:std::sys::unix::os::exit [/home/lenny/wo>
# 1,936 (55.74%)   548 (61.64%)   347 (62.08%)  90 (62.94%) 21 (70.00%)  4 (57.14%)  73 (62.93%) 3 (75.00%) 1 (25.00%)  ???:exit [/usr/lib/libc.so.6]
# 1,928 (55.51%)   547 (61.53%)   345 (61.72%)  88 (61.54%) 21 (70.00%)  4 (57.14%)  71 (61.21%) 3 (75.00%) 1 (25.00%)  ???:0x0000000000040b70 [/usr/lib/libc.so.6]
# 1,198 (34.49%)   321 (36.11%)   202 (36.14%)  49 (34.27%) 13 (43.33%)  4 (57.14%)  32 (27.59%) 0          1 (25.00%)  ???:0x0000000000004d70 [/usr/lib/ld-linux-x86-64.so.2]
while (<STDIN>) {

    # Sort out the lines which are not in the format as shown above
    /^\s*([0-9,]+).*?([\/\?].*)$/ or next;
    my $count  = "$1";
    my $source = "$2";

    # We don't show '???:'
    $source =~ s/^\?\?\?://;

    # callgrind_annotate doesn't make paths relative for all shown paths. But
    # since we do, we need to strip the pwd prefix from the absolute paths.
    $source =~ s/\Q$pwd\E//;
    $source =~ s:^/rustc/([a-zA-Z0-9]{8})[a-zA-Z0-9]+/[/]?(.*)$:/rustc/$1/$2:;

    # Sometimes callgrind_annotate shows a '.' for a count instead of a 0
    $count =~ s/\./0/;

    # The thousands separator needs be removed, since we don't show these
    $count =~ s/,//;

    # We do not show stack lines with costs higher than the sentinel's cost
    if ( $source =~ /^$sentinel / or /^[^:]+:$sentinel / ) {
        $sentinel_source = $source;
        $sentinel_count  = $count;

        # Show some debugging output
        print STDERR "Found sentinel: '$sentinel_source'\n";
    }

    if ( defined $missing_ob and not $source =~ / \[.*\]$/ ) {
        $source .= " [$missing_ob]";
    }

    # We also don't show [???] for unknown elf object filenames
    $source =~ s/ \[\?\?\?\]//;

    for my $i (@replace) {
        if ( "$i->[0]" eq "$source" ) {
            $source = "$i->[1]";
            last;
        }
    }

    for my $i (@modify) {
        if ( "$i->[0]" eq "$source" ) {
            $count = $i->[1];
            last;
        }
    }

    my @rec = ( $source, $count );
    push( @sources, \@rec );
}

if (@insert) {
    for my $i (@insert) {
        my @rec = ( $i->[1], $i->[2] );
        splice @sources, $i->[0], 0, \@rec;
    }
}

# callgrind_annotate per default sorts by event 0 -> event 1 -> ... ->
# lexicographically. We sort by the first event of choice (for example event 0)
# and then lexicographically. So, we need to adjust the sorting of the
# callgrind_annotate output a little bit. This mostly affects the sorting of
# lines with equal costs.
my @sorted_sources = sort { ( $b->[1] <=> $a->[1] ) || ( $a->[0] cmp $b->[0] ) } @sources;

# Create the stacks in flamegraph stacks format from the sorted callgrind_annotate output lines
if (@sources) {
    if ( @sources > 1 ) {
        for my $i ( 0 .. @sorted_sources - 2 ) {
            my $s1 = $sorted_sources[$i];
            my $s2 = $sorted_sources[ $i + 1 ];

            if ( defined $sentinel_count and $s1->[1] > $sentinel_count ) {
                next;
            }

            my $last  = $stacks[-1];
            my $count = $s1->[1] - $s2->[1];
            if ( defined $last ) {
                $last =~ s/ [0-9]+$//;
                push @stacks, "$last;$s1->[0] $count";
            }
            else {
                push @stacks, "$s1->[0] $count";
            }

            if ( $i == @sources - 2 ) {
                my $last  = $stacks[-1];
                my $count = $s2->[1];
                $last =~ s/ [0-9]+$//;
                push @stacks, "$last;$s2->[0] $count";
            }
        }
    }
    else {
        push @stacks, "$sorted_sources[0]->[0] $sorted_sources[0]->[1]";
    }
}

# Finally, print the stacks to stdout
foreach my $stack (@stacks) {
    print "$stack\n";
}
