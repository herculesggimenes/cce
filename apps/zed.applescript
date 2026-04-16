on run argv
  my launch_zed(argv)
end run

on open dropped_items
  set argv to {}
  repeat with item_ref in dropped_items
    set end of argv to POSIX path of item_ref
  end repeat
  my launch_zed(argv)
end open

on open location zed_url
  my launch_zed({zed_url})
end open location

on launch_zed(argv)
  set launcher to POSIX path of (path to home folder) & ".local/bin/zed"
  set quoted_command to quoted form of launcher
  repeat with arg_value in argv
    set quoted_command to quoted_command & space & quoted form of (arg_value as text)
  end repeat
  do shell script quoted_command & " >/dev/null 2>&1 &"
end launch_zed
