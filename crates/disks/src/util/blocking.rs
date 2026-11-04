use crate::error::StorageError;

pub(crate) async fn run_blocking<T, Work>(work: Work) -> Result<T, StorageError>
where
    T: Send + 'static,
    Work: FnOnce() -> Result<T, StorageError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| StorageError::Io(std::io::Error::other(error)))?
}
