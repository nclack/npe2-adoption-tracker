# npe2-adoption-tracker
very hacky attempt at assessing npe2 adoption

### Method

1. Get a list of plugins via api.napari-hub.org
2. Filter by date
   - Napari 0.4.13 release was 17 Jan 2022
   - Look at all plugins with “release_date” after 01 Feb 2022
   - Fudge the date by two weeks to allow for plugins started just before the 0.4.13 to get released.
3. For each plugin
   1. Get repository link
   2. Attempt to resolve repository link to find one or both of setup.py and setup.cfg
   3. Count as an npe2 plugin if either has a reference to napari.yml or napari.yaml
   4. Count as a successfully examined plugin if there were no errors.
4. Report (total detected npe2 plugins)/(total successfully examined plugins)

