# fs-tail

tail a file and block until more lines are added.

## usage


```
let file = std::fs::File::open("/path/to/some/file").unwrap();
let file = TailedFile::new(file);
let locked = file.lock();
for line in locked.lines() {
    if let Ok(line) = line {
        println!("{}", line);
    }
}
```