on run argv
  my launch_cce(argv)
end run

on open dropped_items
  set argv to {}
  repeat with item_ref in dropped_items
    set end of argv to POSIX path of item_ref
  end repeat
  my launch_cce(argv)
end open

on path_is_executable(posix_path)
  try
    do shell script "/bin/test -x " & quoted form of posix_path
    return true
  on error
    return false
  end try
end path_is_executable

on resolve_launcher()
  set home_path to POSIX path of (path to home folder)
  set candidate_paths to {home_path & ".cargo/bin/cce", home_path & ".local/bin/cce", home_path & "src/cce/target/release/cce", home_path & "src/cce/target/debug/cce"}

  repeat with candidate_ref in candidate_paths
    set candidate_path to candidate_ref as text
    if my path_is_executable(candidate_path) then
      return candidate_path
    end if
  end repeat

  return missing value
end resolve_launcher

on notify_failure(message_text)
  try
    display notification message_text with title "CCE"
  end try
end notify_failure

on launch_cce(argv)
  set launcher to my resolve_launcher()
  if launcher is missing value then
    my notify_failure("cce launcher not found. Install the Rust binary first.")
    return
  end if

  set home_path to POSIX path of (path to home folder)
  set log_path to home_path & "Library/Logs/cce-app.log"
  do shell script "/bin/mkdir -p " & quoted form of (home_path & "Library/Logs")

  set quoted_command to "nohup " & quoted form of launcher & " open"
  repeat with arg_value in argv
    set quoted_command to quoted_command & space & quoted form of (arg_value as text)
  end repeat

  try
    do shell script "/bin/sh -lc " & quoted form of (quoted_command & " >>" & quoted form of log_path & " 2>&1 &")
  on error err_msg number err_num
    do shell script "/bin/sh -lc " & quoted form of ("printf '%s\\n' " & quoted form of ("CCE.app launch failed (" & err_num & "): " & err_msg) & " >>" & quoted form of log_path)
    my notify_failure("cce launch failed. See ~/Library/Logs/cce-app.log")
  end try
end launch_cce
