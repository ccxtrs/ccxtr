macro_rules! properties {
    ($($key:ident: $value:expr),*) => {
        {
            let mut properties = Properties::new();
            $(
                properties.insert(stringify!($key), $value);
            )*
            properties
        }
    };
}