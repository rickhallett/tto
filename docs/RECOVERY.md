# Recovery

You turned them off. Now you want them back before the time is up.

First: that is exactly the moment this was built for. Whatever it is, it will
still be there at the time you chose. Close the lid.

Still here? Then this is the procedure. It is slow on purpose. It requires a
restart into macOS Recovery, which takes about ten minutes end to end, and by
step 4 most people find they no longer want step 6.

1. Save your work. Shut down the Mac.
2. Start it up in Recovery: on Apple silicon, hold the power button until
   "Loading startup options" appears, choose **Options**, then **Continue**.
3. From the menu bar choose **Utilities → Terminal**.
4. Find your data volume: `ls /Volumes`. It is usually `Macintosh HD` or
   `Macintosh HD - Data`. Tell the shell which one, exactly as listed:
   ```
   V='Macintosh HD - Data'
   ```
5. Remove the helper and its memory:
   ```
   rm "/Volumes/$V/Library/LaunchDaemons/dev.oceanheart.tto.plist"
   rm "/Volumes/$V/Library/PrivilegedHelperTools/dev.oceanheart.tto"
   chflags -R noschg "/Volumes/$V/private/var/db/tto"
   rm -r "/Volumes/$V/private/var/db/tto"
   ```
6. Put the hosts file back: open `/Volumes/$V/private/etc/hosts` in `nano`
   and delete everything from the line `# >>> tto` to the line `# <<< tto <<<`.
7. Restart normally.

That's it. Nothing else was changed, and nothing phones home. If you got this
far and did it anyway, no judgement. Maybe pick "tonight" next time instead
of "the weekend".

An admin user with a terminal can do the same from a normal session with
`sudo`, in about five commands, if they know which five. We are not going to
list them here. Not because it's secret, but because looking them up at 2am
is the friction that makes the tool work.
