#[cfg(test)]
#[macro_use]
mod test_utils {
    use std::path::Path;

    pub fn touch(path: &Path) {
        std::fs::File::create(path).unwrap();
    }

    pub fn mkdir(path: &Path) {
        std::fs::create_dir(path).unwrap();
    }

    macro_rules! create_tree {
        ($path:expr, { $file:ident, $($rest:tt)* }) => {
            let path = $path;
            $crate::utils::touch(&path.join(stringify!($file)));
            create_tree!(path, { $($rest)* })
        };
        ($path:expr, { $dir:ident: { $($children:tt)* }, $($rest:tt)* }) => {
            let path = $path;
            let dir = path.join(stringify!($dir));
            $crate::utils::mkdir(&dir);
            create_tree!(&dir, { $($children)* });
            create_tree!(path, { $($rest)* })
        };
        ($path:expr, { }) => {};
    }

    macro_rules! test_tree {
        ($path:expr, { $file:ident: ($is_ty:ident, $symlink_is_ty:ident), $($rest:tt)* }) => {
            let path = $path;
            assert!(path.join(stringify!($file)).metadata().unwrap().$is_ty());
            assert!(path.join(stringify!($file)).symlink_metadata().unwrap().$symlink_is_ty());
            test_tree!(path, { $($rest)* })
        };
        ($path:expr, { $dir:ident: { $($children:tt)* }, $(, $rest:tt)* }) => {
            let path = $path;
            let dir = path.join(stringify!($dir));
            test_tree!(&dir, { $($children)* });
            test_tree!(path, { $($rest)* })
        };
        ($path:expr, {}) => {};
    }
}

#[cfg(test)]
pub(crate) use test_utils::*;
