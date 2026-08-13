use std::{path::PathBuf, time::Instant};

use atha_backend::reader::dictionary::{DictionaryError, DictionaryLookup, LocalDictionary};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{ReaderRuntime, begin_local_data_operation, platform_file};

const MAX_IMPORT_FILES: usize = 5;
const MAX_PICKER_BYTES: u64 = 256 * 1024 * 1024;

#[tauri::command]
pub(crate) fn list_local_dictionaries(
    runtime: State<'_, ReaderRuntime>,
) -> Result<Vec<LocalDictionary>, String> {
    let _operation = begin_local_data_operation(&runtime)?;
    let started = Instant::now();
    runtime
        .dictionaries
        .list()
        .map_err(|error| command_error("list", "records", &started, error))
}

#[tauri::command]
pub(crate) async fn import_local_dictionary(
    app: AppHandle,
    runtime: State<'_, ReaderRuntime>,
) -> Result<Option<Vec<LocalDictionary>>, String> {
    let Some(selected) = app
        .dialog()
        .file()
        .add_filter("MDict / Kindle 词典", &["mdx", "mdd", "mobi"])
        .blocking_pick_files()
    else {
        return Ok(None);
    };
    if selected.is_empty() || selected.len() > MAX_IMPORT_FILES {
        return Err("invalid-dictionary-import".into());
    }
    let inputs = selected
        .into_iter()
        .map(|path| {
            platform_file::PickerInput::open_dictionary(&app, path, MAX_PICKER_BYTES)
                .map_err(|_| "invalid-dictionary-source".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mdx = inputs
        .iter()
        .filter(|input| input.suffix() == "mdx")
        .collect::<Vec<_>>();
    let kindle = inputs
        .iter()
        .filter(|input| input.suffix() == "mobi")
        .collect::<Vec<_>>();
    if !((mdx.len() == 1 && kindle.is_empty())
        || (kindle.len() == 1 && mdx.is_empty() && inputs.len() == 1))
    {
        return Err("invalid-dictionary-import".into());
    }
    let mdx = mdx.first().map(|input| input.path().to_path_buf());
    let kindle = kindle.first().map(|input| input.path().to_path_buf());
    let resources = inputs
        .iter()
        .filter(|input| input.suffix() == "mdd")
        .map(|input| input.path().to_path_buf())
        .collect::<Vec<PathBuf>>();
    let _operation = begin_local_data_operation(&runtime)?;
    let dictionaries = runtime.dictionaries.clone();
    let started = Instant::now();
    tauri::async_runtime::spawn_blocking(move || {
        let format = if let Some(mdx) = mdx {
            dictionaries
                .import_mdict(mdx, &resources)
                .map_err(|error| command_error("import", "backend", &started, error))?;
            "mdict"
        } else if let Some(kindle) = kindle {
            dictionaries
                .import_kindle(kindle)
                .map_err(|error| command_error("import", "backend", &started, error))?;
            "kindle-mobi6"
        } else {
            return Err("invalid-dictionary-import".into());
        };
        let result = dictionaries
            .list()
            .map_err(|error| command_error("import", "list", &started, error))?;
        log::info!(
            target: "atha::dictionary",
            "operation=import outcome=ok format={} resource_count={} duration_ms={}",
            format,
            resources.len(),
            started.elapsed().as_millis()
        );
        Ok(Some(result))
    })
    .await
    .map_err(|_| "dictionary-import-task".to_owned())?
}

#[tauri::command]
pub(crate) async fn lookup_local_dictionary(
    runtime: State<'_, ReaderRuntime>,
    dictionary_id: String,
    query: String,
) -> Result<Option<DictionaryLookup>, String> {
    let _operation = begin_local_data_operation(&runtime)?;
    let dictionaries = runtime.dictionaries.clone();
    let started = Instant::now();
    tauri::async_runtime::spawn_blocking(move || {
        let result = dictionaries
            .lookup(&dictionary_id, &query)
            .map_err(|error| command_error("lookup", "backend", &started, error))?;
        log::info!(
            target: "atha::dictionary",
            "operation=lookup outcome=ok result_count={} duration_ms={}",
            usize::from(result.is_some()),
            started.elapsed().as_millis()
        );
        Ok(result)
    })
    .await
    .map_err(|_| "dictionary-lookup-task".to_owned())?
}

#[tauri::command]
pub(crate) fn remove_local_dictionary(
    runtime: State<'_, ReaderRuntime>,
    dictionary_id: String,
) -> Result<Vec<LocalDictionary>, String> {
    let _operation = begin_local_data_operation(&runtime)?;
    let started = Instant::now();
    runtime
        .dictionaries
        .remove(&dictionary_id)
        .map_err(|error| command_error("remove", "directory", &started, error))?;
    runtime
        .dictionaries
        .list()
        .map_err(|error| command_error("remove", "list", &started, error))
}

fn command_error(
    operation: &'static str,
    stage: &'static str,
    started: &Instant,
    error: DictionaryError,
) -> String {
    let code = error.code();
    if is_internal_error(error) {
        log::error!(
            target: "atha::dictionary",
            "operation={operation} stage={stage} outcome=failed code={code} duration_ms={}",
            started.elapsed().as_millis()
        );
    }
    code.into()
}

const fn is_internal_error(error: DictionaryError) -> bool {
    matches!(
        error,
        DictionaryError::InvalidRoot
            | DictionaryError::CorruptRecord
            | DictionaryError::ReadFailed
            | DictionaryError::WriteFailed
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_internal_dictionary_failures_are_log_worthy() {
        for error in [
            DictionaryError::InvalidRoot,
            DictionaryError::CorruptRecord,
            DictionaryError::ReadFailed,
            DictionaryError::WriteFailed,
        ] {
            assert!(is_internal_error(error));
        }
        for error in [
            DictionaryError::InvalidSource,
            DictionaryError::SourceTooLarge,
            DictionaryError::TooManyResources,
            DictionaryError::Unsupported,
            DictionaryError::CorruptSource,
            DictionaryError::InvalidDictionaryId,
            DictionaryError::UnknownDictionary,
            DictionaryError::InvalidQuery,
            DictionaryError::DefinitionTooLarge,
            DictionaryError::ResourceTooLarge,
            DictionaryError::LinkDepth,
        ] {
            assert!(!is_internal_error(error));
        }
    }
}
