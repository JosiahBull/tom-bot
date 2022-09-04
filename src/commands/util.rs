//! Various utilities to assist with writing application commands for the DIANA bot

use log::{debug, error, info, warn};
use serenity::{
    builder::{CreateInteractionResponse}, model::prelude::{interaction::InteractionResponseType},
};

use crate::AppState;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code, clippy::missing_docs_in_private_items)]
pub enum FailureMessageKind {
    Error,
    Warn,
    Info,
    Debug,
}

/// a general purpose response type generated by the bot reacting to a slash command
/// has both basic and complex success and failure states
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum CommandResponse<'a> {
    /// a basic success, will return the contained string in a simple message to the user
    BasicSuccess(String),
    /// a complex success, will return the contained interaction response exactly to the user
    ComplexSuccess(CreateInteractionResponse<'a>),
    /// a basic failure, will return the contained string in a simple message to the user
    /// and log the message to the console
    BasicFailure(String),
    /// a complex failure, will return the contained interaction response exactly to the user
    /// and log the message to the console with the provided log level
    ComplexFailure {
        /// the string to send to the user
        response: String,
        /// the log level to use for the message to the console
        kind: FailureMessageKind,
        /// the message to send to the console
        log_message: String,
    },
    /// represents an internal failure, will NOT send the contained string to the user
    /// but will instead log it to the console, and return a generic "internal error" resposne
    /// to the user
    InternalFailure(String),
}

impl<'a> CommandResponse<'a> {
    /// Get the log message to write to the console, if it exists
    pub fn get_log_message(&self) -> Option<&str> {
        match self {
            Self::BasicFailure(message) => Some(message),
            Self::ComplexFailure { log_message, .. } => Some(log_message),
            Self::InternalFailure(message) => Some(message),
            _ => None,
        }
    }

    /// Get the log level to use when logging the message
    pub fn get_log_type(&self) -> FailureMessageKind {
        match self {
            Self::BasicFailure(_) => FailureMessageKind::Error,
            Self::ComplexFailure { kind, .. } => *kind,
            _ => FailureMessageKind::Info,
        }
    }

    /// writ ethe message to the log, if there is a loggable message
    pub fn write_to_log(&self) {
        if let Some(message) = self.get_log_message() {
            match self.get_log_type() {
                FailureMessageKind::Error => error!("{}", message),
                FailureMessageKind::Warn => warn!("{}", message),
                FailureMessageKind::Info => info!("{}", message),
                FailureMessageKind::Debug => debug!("{}", message),
            }
        }
    }

    /// generate a response to be sent to the user from the CommandResponse type
    pub fn generate_response(self) -> CreateInteractionResponse<'a> {
        match self {
            CommandResponse::BasicSuccess(message) => CreateInteractionResponse::default()
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|data| data.ephemeral(true).content(message))
                .to_owned(),
            CommandResponse::ComplexSuccess(message) => message,
            CommandResponse::BasicFailure(message) => CreateInteractionResponse::default()
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|data| data.ephemeral(true).content(message))
                .to_owned(),
            CommandResponse::ComplexFailure { response, .. } => {
                CreateInteractionResponse::default()
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|data| data.ephemeral(true).content(response))
                    .to_owned()
            }
            CommandResponse::InternalFailure(_) => CreateInteractionResponse::default()
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|data| {
                    data.ephemeral(true).content("An internal error occurred.")
                })
                .to_owned(),
        }
    }
}

/////////// HELPER FUNCTIONS ///////////

