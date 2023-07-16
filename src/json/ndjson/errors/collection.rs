use std::cell::RefCell;
use std::error::Error;
use std::fmt::Display;
use std::iter::Enumerate;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::{fmt, io};

use owo_colors::{OwoColorize, Stream};

use super::NDJSONError;

/// Holds linked position information for errors encountered while processing
#[derive(Debug)]
pub struct IndexedNDJSONError {
    pub location: String,
    pub error: NDJSONError,
}

impl IndexedNDJSONError {
    pub(crate) fn new(location: String, error: NDJSONError) -> Self {
        Self { location, error }
    }
}

impl fmt::Display for IndexedNDJSONError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Line {}: {}", self.location, self.error)?;
        if let Some(source) = self.error.source() {
            write!(f, "; {source}")?;
        }
        Ok(())
    }
}

/// Threadsafe storage for errors enounter by parallel processing.
/// Counterpart to [`Errors`]
#[derive(Debug)]
pub struct ErrorsPar<E> {
    pub container: Arc<Mutex<Vec<E>>>,
}

impl<E> ErrorsPar<E> {
    pub fn new(container: Arc<Mutex<Vec<E>>>) -> Self {
        Self { container }
    }

    pub fn new_ref(&self) -> Self {
        Self {
            container: Arc::clone(&self.container),
        }
    }

    pub fn push(&self, value: E) {
        self.container.lock().expect("not poisoned").push(value)
    }
}

impl<E> Default for ErrorsPar<E> {
    fn default() -> Self {
        Self::new(Arc::new(Mutex::new(vec![])))
    }
}

impl<E: Display> fmt::Display for ErrorsPar<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in self.container.lock().unwrap().as_slice() {
            writeln!(f, "{i}")?;
        }
        Ok(())
    }
}

impl<E: Display> ErrorsPar<E> {
    pub fn eprint(&self) {
        let stream = Stream::Stdout;
        if !self.container.lock().unwrap().is_empty() {
            eprintln!("{}", self.if_supports_color(stream, |text| text.red()));
        }
    }
}

// TODO: Create ErrorContainer Trait?
/// Storage for errors enounter by processing
/// Counterpart to [`ErrorsPar`]
#[derive(Debug)]
pub struct Errors<E> {
    pub container: Rc<RefCell<Vec<E>>>,
}

impl<E> Errors<E> {
    pub fn new(container: Rc<RefCell<Vec<E>>>) -> Self {
        Self { container }
    }

    pub fn new_ref(&self) -> Self {
        Self {
            container: Rc::clone(&self.container),
        }
    }

    pub fn push(&self, value: E) {
        self.container.borrow_mut().push(value)
    }
}

impl<E: Display> Errors<E> {
    pub fn eprint(&self) {
        let stream = Stream::Stdout;
        if !self.container.borrow().is_empty() {
            eprintln!("{}", self.if_supports_color(stream, |text| text.red()));
        }
    }
}

impl<E> Default for Errors<E> {
    fn default() -> Self {
        Self::new(Rc::new(RefCell::new(vec![])))
    }
}

impl<E: Display> fmt::Display for Errors<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in self.container.borrow().as_slice() {
            writeln!(f, "{i}")?;
        }
        Ok(())
    }
}

/// Iterator that skips, but keeps track of, `Err`s while processing
pub struct ErrFiltered<I, E> {
    iter: I,
    errors: Errors<E>,
}

impl<E, T, I: Iterator<Item = (String, Result<T, W>)>, W> ErrFiltered<I, E> {
    pub fn new(iter: I, errors: Errors<E>) -> Self {
        Self { iter, errors }
    }
}

impl<T, I> Iterator for ErrFiltered<I, IndexedNDJSONError>
where
    I: Iterator<Item = (String, Result<T, serde_json::Error>)>,
{
    type Item = (String, T);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (id, next_item) = self.iter.next()?;
            match next_item {
                Ok(item) => break Some((id, item)),
                Err(e) => {
                    self.errors.push(IndexedNDJSONError::new(
                        id,
                        NDJSONError::JSONParsingError(e),
                    ));
                }
            }
        }
    }
}

pub trait IntoErrFiltered<E, T, W>: Iterator<Item = (String, Result<T, W>)> + Sized {
    fn to_err_filtered(self, errors: Errors<E>) -> ErrFiltered<Self, E> {
        ErrFiltered::new(self, errors)
    }
}

impl<E, T, I: Iterator<Item = (String, Result<T, W>)>, W> IntoErrFiltered<E, T, W> for I {}

/// Iterator that enumerates all items and skips, but keeps track of, `Err`s while processing
pub struct EnumeratedErrFiltered<I, E> {
    iter: Enumerate<I>,
    errors: Errors<E>,
}

impl<E, T, I: Iterator<Item = Result<T, W>>, W> EnumeratedErrFiltered<I, E> {
    pub fn new(iter: I, errors: Errors<E>) -> Self {
        Self {
            iter: iter.enumerate(),
            errors,
        }
    }
}

impl<T, I> Iterator for EnumeratedErrFiltered<I, IndexedNDJSONError>
where
    I: Iterator<Item = Result<T, io::Error>>,
{
    type Item = (usize, T);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (i, next_item) = self.iter.next()?;
            let i = i + 1; // count lines from 1
            match next_item {
                Ok(item) => break Some((i, item)),
                Err(e) => {
                    self.errors.push(IndexedNDJSONError::new(
                        i.to_string(),
                        NDJSONError::IOError(e),
                    ));
                }
            }
        }
    }
}

pub trait IntoEnumeratedErrFiltered<E, T, W>: Iterator<Item = Result<T, W>> + Sized {
    fn to_enumerated_err_filtered(self, errors: Errors<E>) -> EnumeratedErrFiltered<Self, E> {
        EnumeratedErrFiltered::new(self, errors)
    }
}

impl<E, T, I: Iterator<Item = Result<T, W>>, W> IntoEnumeratedErrFiltered<E, T, W> for I {}
