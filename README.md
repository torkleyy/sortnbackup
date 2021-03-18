# sortnbackup

## CLI

````text
sortnbackup
Copy files from multiple sources to multiple targets using highly customizable filters and rules

USAGE:
    sortnbackup [FLAGS]

FLAGS:
    -c, --continue    Continue a previously started backup
    -h, --help        Prints help information
    -V, --version     Prints version information
        --yes         Answer all questions with yes (non-interactive mode)
````

## `config.yaml`

```yaml
settings:
  file_size_style: binary # for console output; binary (MiB) or decimal (MB)

sources:
  usb_stick:
    ignore_paths:
      - cache
      - thumbnails
    path: "F:\\"
  home_dir:
    ignore_paths:
      - AppData
    path: "C:\\Users\\John"
  media_videos:
    path: "A:\\Videos"
    disabled: true

targets:
  external_hdd: "H:\\Backup"

# evaluated in order, first matching filter determines file group for a file
file_groups:
  # file group name is just for descriptive / debugging purposes
  ignore_hidden_files_and_folders: # ignore all files tarting with .
    sources: all
    filter:
      file_name_matches_regex: "^\\."
    rule: ignore
  videos:
    sources: all
    filter:
      all:
        - is_file
        - has_extension: [mp4, mkv]
    rule:
      copy_to:
        target: "external_hdd"
        path:
          - file_name: "Videos"
          - file_name_with_extension
  documents:
    sources: all
    filter:
      all:
        - is_file
        - any:
            - in_folder: "Documents"
            - has_extension: [ doc, docx, pdf ]
    rule:
      copy_to:
        target: "external_hdd"
        path:
          - file_name: "Documents"
          - created_time: "%Y-%m-%d"
          - file_name_with_extension
  json:
    sources: all
    filter:
      all:
        - is_file
        - has_extension: [ json ]
    rule:
      copy_to:
        target: "external_hdd"
        path:
          - file_name: "JSON"
          - created_time: "%Y-%m-%d"
          - file_name_with_extension
  image_no_thumb: # copy all images with a size of 300+ to external_hdd's Image folder
    sources: all
    filter:
      all:
        - is_file
        - has_extension: [ png, jpg, jpeg ]
        - has_img_metadata # image metadata can be expensive to read, so put this filter as far down as possible
        - has_img_date_time # implies "has_img_metadata", and should be used if img_date_time is used in rule
        - img_size:
            min: 300
    rule:
      copy_to:
        target: "external_hdd"
        path:
          - file_name: "Images"
          - img_date_time: "%Y-%m"
          - img_date_time: "%d"
          - file_name_with_extension
  image_no_thumb_no_time: # also copy images without date / time info
    sources: all
    filter:
      all:
        - is_file
        - has_extension: [ png, jpg, jpeg ]
        - has_img_metadata # image metadata can be expensive to read, so put this filter as far down as possible
        - img_size:
            min: 300
    rule:
      copy_to:
        target: "external_hdd"
        path:
          - file_name: "Images"
          - file_extension
          - file_name_with_extension
  thumbnails: # copy thumbnails (all remaining images)
    sources: all
    filter:
      all:
        - is_file
        - has_extension: [ png, jpg, jpeg ]
        - has_img_metadata # image metadata can be expensive to read, so put this filter as far down as possible
    rule:
      copy_to:
        target: "external_hdd"
        path:
          - file_name: "Thumbnails"
          - file_extension
          - file_name_with_extension
  misc: # match all files that had no other filter matching them (otherwise they would be ignored)
    sources: all
    filter: is_file
    rule:
      copy_to: # default would be "ignore" (if no file group would match)
        target: "external_hdd"
        path:
          - file_name: "Misc"
          - original_path
  traverse_folders: # match all folders that had no other filter matching them
    sources: all
    filter: is_dir
    rule: traverse # default is "traverse" (so this group could be deleted without any effect)
```

An alternative to `traverse_folders`:

```yaml
# ...
file_groups:
  traverse_media_folders:
    sources: all
    filter:
      all:
        - is_dir
        - any:
          - file_name: "Images"
          - file_name: "Photos"
          - file_name: "Videos"
    rule: Traverse
  # ...
  # this should be the last rule
  copy_other_folders:
    sources: all
    filter: is_dir
    copy_exact:
      target: "external_hdd"
```

### Filters

#### `all`

Require all filters to match:

```yaml
all:
  - filter1
  - filter2
```

#### `any`

Require one or more filters to match:

```yaml
any:
  - filter1
  - filter2
```

#### `not`

Matches if the inner filter does not match:

```yaml
not: filter1
```

#### `catch_all`

Always matches:

```yaml
catch_all
```

#### `in_folder`

Matches all files/directories that are inside the specified folder.

```yaml
in_folder: "Documents/Invoices"
```

#### `directly_in_folder`

Matches all files/directories that are directly inside the specified folder.
I.e., "Documents\Invoices\sub-folder\document.pdf" does not match this filter.

```yaml
directly_in_folder: "Documents/Invoices"
```

#### `has_extension`

Matches all files/directories with the given extension.
[Definition of "extension"](https://doc.rust-lang.org/std/path/struct.Path.html#method.extension)

```yaml
has_extension: [ txt, doc, docx ]
```

#### `file_name`

Matches all files/directories with the specified file name (case-insensitive):

```yaml
file_name: "Documents"
```

#### `file_name_matches_regex`

Matches all files/directories with a name matching the regex:

```yaml
file_name_matches_regex: "^\\." # all files/folders starting with .
```

#### `path_matches_regex`

Matches all files/directories with a path matching the regex:

```yaml
path_matches_regex: "^\\." # all files/folders starting with .
```

#### `has_img_date_time`

Matches all files with image metadata including date/time information.

Implies:
* `has_img_metadata`

```yaml
has_img_date_time
```

#### `has_img_metadata`

Matches all files with image metadata.

```yaml
has_img_metadata
```

#### `is_file`

Matches all files.

```yaml
is_file
```

#### `is_dir`

Matches all directories.

```yaml
is_dir
```

#### `img_size`

Matches all image files with a given min / max pixel size.

```yaml
img_size:
  min: 300 # optional (if ~ or not specified, there's no limit)
  max: ~ # optional (if ~ or not specified, there's no limit)
```

### Path Elements

#### `file_name`

A constant folder or file name:

```yaml
file_name: "log.txt"
```

```yaml
file_name: "Images"
```

#### `merge_strings`

Merge multiple path elements together to generate a single file name / directory.
If path elements were written out in flat, each element generated a directory.

```yaml
merge_strings:
  - file_name: "file_"
  - file_name_without_extension
  - file_name: "_"
  - file_extension
  - file_name: "."
  - file_extension
```

#### `original_path`

The path of the file / directory relative to the source.

```yaml
original_path
```

#### `original_path_without_file_name`

The path of the parent directory of the file / directory relative to the source.

```yaml
original_path_without_file_name
```

#### `direct_parent_folder`

The name of the parent directory of the file / directory.

```yaml
direct_parent_folder
```

#### `file_name_with_extension`

The name of the file / directory including the extension.

```yaml
file_name_with_extension
```

#### `file_name_without_extension`

The name of the file / directory without extension.

```yaml
file_name_without_extension
```

#### `file_extension`

The extension of the file / directory.

```yaml
file_extension
```

#### `img_date_time`

The date / time of the image.
[Formatting symbols](https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html#specifiers)

```yaml
img_date_time: "%Y-%m-%d"
```

#### `access_time`

The access time of the file / folder.
[Formatting symbols](https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html#specifiers)

```yaml
access_time: "%Y-%m-%d"
```

#### `created_time`

The creation time of the file / folder.
[Formatting symbols](https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html#specifiers)

```yaml
created_time: "%Y-%m-%d"
```

#### `modified_time`

The modification time of the file / folder.
[Formatting symbols](https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html#specifiers)

```yaml
modified_time: "%Y-%m-%d"
```

## License

> MIT OR Apache-2.0.
