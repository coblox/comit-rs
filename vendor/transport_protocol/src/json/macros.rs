#[macro_export]
macro_rules! try_header {
    ($e:expr) => {
        header_internal!($e, {
            let value = Default::default();

            info!(
                "Header was not present, falling back to default value: '{:?}'",
                value
            );

            value
        })
    };
}

#[macro_export]
macro_rules! header {
    ($e:expr) => {
        header_internal!($e, {
            info!("Header was not present, early returning with error response (SE00)!");
            return Response::new(Status::SE(0));
        })
    };
}

#[macro_export]
macro_rules! body {
    ($e:expr) => {
        match $e {
            Ok(body) => body,
            Err(e) => {
                error!("Failed to deserialize body: {:?}", e);
                return Response::new(Status::SE(0))
            }
        }
    };
}

#[macro_export]
macro_rules! header_internal {
    ($e:expr, $none:expr) => {
        match $e {
            Some(Ok(header)) => header,
            Some(Err(e)) => {
                error!("Failed to deserialize header: {:?}", e);
                return Response::new(Status::SE(0))
            },
            None => $none,
        }
    };
}
