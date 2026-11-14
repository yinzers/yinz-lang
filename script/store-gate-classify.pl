#!/usr/bin/env perl
# Classify the files `ynz` newly rejects (see script/store-gate.sh): for each, every ERROR
# diagnostic of the new run is placed by the source line it points at.
#   let-copy   the line is `let b = a`, `const b: T = a` or `b = a`: one name bound to another
#   store      every other line: a container, field or index store, a container literal
# A file is group (a) when all its errors are let-copy, else group (b). A refused store (a parameter, a piece, a cell) is read at the store line; a store that was admitted and then read is read from the copy form its WHAT INSTEAD quotes.
#
#   perl script/store-gate-classify.pl <gate-dir-new> <newly-rejected-list>
use strict; use warnings;
my ($gate_dir, $list) = @ARGV;
open my $fh, '<', $list or die $!; chomp(my @files = <$fh>); close $fh;
my (%group, %lines);
for my $file (@files) {
  (my $name = $file) =~ s{/}{__}g;
  open my $nd, '<', "$gate_dir/$name.ndjson" or die "$name: $!";
  open my $src, '<', $file or die "$file: $!";
  my $text = do { local $/; <$src> }; close $src;
  my ($let, $store) = (0, 0);
  while (my $line = <$nd>) {
    next unless $line =~ /"severity":"error"/;
    next unless $line =~ /"start_byte":(\d+)/;
    my $start = $1;
    my ($span_file) = $line =~ m{"file":"([^"]*)"};
    next unless $span_file =~ /\Q$file\E$/;   # a diagnostic in another file of the project
    my $class;
    if ($line =~ /was stored in/) {
      # The span is the LATER READ; the store itself is quoted in WHAT INSTEAD.
      my ($copy_form) = $line =~ /store a copy instead: `([^`]*)`/;
      $class = ($copy_form // "") =~ /^(?:let |const )?\w+ = / ? "let" : "store";
    }
    my $from = rindex($text, "\n", $start) + 1;
    my $to = index($text, "\n", $start); $to = length($text) if $to < 0;
    my $source_line = substr($text, $from, $to - $from);
    $class //= $source_line =~ /^\s*(?:let|const)?\s*\w+\s*(?::\s*[^=]+?)?=\s*\w+\s*(?:\/\/.*)?$/ ? "let" : "store";
    if ($class eq "let") { $let++ } else { $store++ }
    push @{ $lines{$file} }, $source_line =~ s/^\s+//r;
  }
  $group{$file} = ($store == 0 && $let > 0) ? 'a' : 'b';
}
for my $g ('a', 'b') {
  my @in = grep { $group{$_} eq $g } @files;
  my $bytes = 0; $bytes += -s $_ for @in;
  printf "group (%s): %d files, %d bytes\n", $g, scalar(@in), $bytes;
  print "  $_\n" for @in;
}
