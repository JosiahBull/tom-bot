use std::{cmp::Ordering, collections::HashSet};

use log::error;
use serenity::{
    all::{
        AutocompleteOption, ChannelId, CommandInteraction, CommandOptionType, ComponentInteraction,
        GuildId, Message, ResolvedValue,
    },
    async_trait,
    builder::{
        AutocompleteChoice, CreateActionRow, CreateAutocompleteResponse, CreateButton,
        CreateCommand, CreateCommandOption, CreateEmbed, CreateInteractionResponse,
        CreateInteractionResponseFollowup, CreateInteractionResponseMessage, CreateMessage,
        EditMessage,
    },
    prelude::Context,
};

use crate::{
    discord_bot::{
        common::embed::EmbedColor,
        database::shopping::{NewShoppingListItem, SerenityShoppingDatabase},
    },
    state::AppState,
};

use super::{
    command::{AutocompleteCommand, Command, InteractionCommand},
    util::CommandResponse,
};

const EXTRA_STORE_NAMES: &[&str] = &[
    "Pack'n'Save",
    "Countdown",
    "Bunnings",
    "Mitre 10",
    "The Warehouse",
    "Kmart",
    "Farmers",
];

const EXTRA_ITEMS: &[&str] = &[
    "milk 2L",
    "loaf of bread",
    "12 eggs",
    "cheese 1kg",
    "butter",
    "chocolate",
    "coffee",
    "tea",
    "sugar",
    "flour",
    "oil",
    "x2 can of tomatoes",
    "fresh tomatoes",
    "cherry tomatoes",
    "brown onions",
    "red onions",
    "potatoes",
    "carrots",
    "general fruit and vege",
    "chicken breast 500g",
    "beef mince 500g",
    "pork mine 500g",
    "white fish",
    "hoki crumbed fish",
    "orange juice (pulp)",
    "orange juice (no pulp)",
    "toilet paper",
    "paper towels",
    "dishwashing liquid",
    "dishwasher powder",
    "washing powder",
    "napisan powder",
    "bleach",
    "toothpaste",
    "toothbrush",
    "shampoo",
    "conditioner",
    "soap",
    "deodorant",
    "razors",
    "shaving cream",
    "hair gel",
    "band-aids",
    "painkillers",
    "antibiotics",
    "vitamins",
    "protein powder",
    "banana",
    "apple",
    "orange",
    "kiwi fruit",
    "lemon",
    "lime",
    "avocado",
    "cucumber",
    "lettuce",
    "capsicum",
    "zucchini",
    "broccoli",
    "cauliflower",
    "asparagus",
    "corn",
    "mushrooms",
    "spinach",
    "tomato",
];

#[async_trait]
trait Interactable: Sync {
    async fn interactable_create_response(
        &self,
        ctx: &Context,
        response: CreateInteractionResponse,
    ) -> Result<(), serenity::Error>;

    async fn interactable_create_followup(
        &self,
        ctx: &Context,
        response: CreateInteractionResponseFollowup,
    ) -> Result<Message, serenity::Error>;

    async fn interactable_get_response(&self, ctx: &Context) -> Result<Message, serenity::Error>;

    fn user(&self) -> &serenity::model::user::User;
    fn channel_id(&self) -> ChannelId;
    fn guild_id(&self) -> Option<GuildId>;
}

#[async_trait]
impl Interactable for CommandInteraction {
    async fn interactable_create_response(
        &self,
        ctx: &Context,
        response: CreateInteractionResponse,
    ) -> Result<(), serenity::Error> {
        self.create_response(ctx, response).await
    }

    async fn interactable_create_followup(
        &self,
        ctx: &Context,
        response: CreateInteractionResponseFollowup,
    ) -> Result<Message, serenity::Error> {
        self.create_followup(ctx, response).await
    }

    async fn interactable_get_response(&self, ctx: &Context) -> Result<Message, serenity::Error> {
        self.get_response(ctx).await
    }

    fn user(&self) -> &serenity::model::user::User {
        &self.user
    }

    fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    fn guild_id(&self) -> Option<GuildId> {
        self.guild_id
    }
}

#[async_trait]
impl Interactable for ComponentInteraction {
    async fn interactable_create_response(
        &self,
        ctx: &Context,
        response: CreateInteractionResponse,
    ) -> Result<(), serenity::Error> {
        self.create_response(ctx, response).await
    }

    async fn interactable_create_followup(
        &self,
        ctx: &Context,
        response: CreateInteractionResponseFollowup,
    ) -> Result<Message, serenity::Error> {
        self.create_followup(ctx, response).await
    }

    async fn interactable_get_response(&self, ctx: &Context) -> Result<Message, serenity::Error> {
        self.get_response(ctx).await
    }

    fn user(&self) -> &serenity::model::user::User {
        &self.user
    }

    fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    fn guild_id(&self) -> Option<GuildId> {
        self.guild_id
    }
}

trait Constructable: Default {
    fn add_embed(self, embed: CreateEmbed) -> Self;
    fn add_components(self, components: Vec<CreateActionRow>) -> Self;
}

impl Constructable for CreateInteractionResponseFollowup {
    fn add_embed(self, embed: CreateEmbed) -> Self {
        self.embed(embed)
    }

    fn add_components(self, components: Vec<CreateActionRow>) -> Self {
        self.components(components)
    }
}

impl Constructable for CreateMessage {
    fn add_embed(self, embed: CreateEmbed) -> Self {
        self.embed(embed)
    }

    fn add_components(self, components: Vec<CreateActionRow>) -> Self {
        self.components(components)
    }
}

async fn create_loading_message<'b, A: Interactable>(
    interaction: &'b A,
    ctx: &'b Context,
) -> Result<u64, CommandResponse> {
    if let Err(e) = interaction
        .interactable_create_response(
            ctx,
            CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new()),
        )
        .await
    {
        return Err(CommandResponse::InternalFailure(format!(
            "error communicating with database: {}",
            e
        )));
    }

    let loading_message = match interaction.interactable_get_response(ctx).await {
        Ok(m) => m,
        Err(e) => {
            return Err(CommandResponse::InternalFailure(format!(
                "error communicating with database: {}",
                e
            )));
        }
    };

    Ok(loading_message.id.into())
}

async fn push_list_item_to_database<'b, A: Interactable>(
    shop: Shop<'b>,
    state: &'b AppState,
    interaction: &'b A,
    ctx: &'b Context,
    message_id: u64,
) -> Result<(), CommandResponse> {
    let user_id = interaction.user().id.into();
    let channel_id = interaction.channel_id().into();
    let guild_id = interaction.guild_id().map(|g| g.0.into());

    if let Err(e) = state
        .add_shopping_list_item(
            user_id,
            message_id,
            channel_id,
            guild_id,
            NewShoppingListItem {
                item: shop.item,
                personal: shop.personal,
                quantity: shop.quantity,
                store: shop.store,
                notes: shop.notes,
            },
        )
        .await
    {
        error!("error adding shopping list item: {}", e);
        if let Err(inner_e) = interaction
            .interactable_create_followup(
                ctx,
                CreateInteractionResponseFollowup::new()
                    .content("error communicating with database")
                    .ephemeral(true),
            )
            .await
        {
            error!("error editing message to return error: {}", inner_e);
        }
        return Err(CommandResponse::NoResponse);
    }
    Ok(())
}

async fn create_new_shopping<'b, B: Constructable>(
    shop: &'b Shop<'b>,
) -> Result<B, CommandResponse> {
    Ok(B::default()
        .add_embed(
            CreateEmbed::new()
                // .title("Added to shopping list") //XXX: experiment
                .description(format!(
                    "Added x{} {}{} to the shopping list{}{}",
                    shop.quantity,
                    shop.item,
                    if shop.personal { " (personal)" } else { "" },
                    if shop.store.is_some() {
                        format!(" from {}", shop.store.unwrap())
                    } else {
                        "".to_string()
                    },
                    if shop.notes.is_some() {
                        format!("\n**note:** {}", shop.notes.unwrap())
                    } else {
                        "".to_string()
                    },
                ))
                .color(EmbedColor::Red as u32),
        )
        .add_components(vec![CreateActionRow::Buttons(vec![
            CreateButton::new("bought")
                .style(serenity::all::ButtonStyle::Success)
                .label("Bought"),
            CreateButton::new("remove")
                .style(serenity::all::ButtonStyle::Danger)
                .label("Remove"),
            CreateButton::new("readd")
                .style(serenity::all::ButtonStyle::Secondary)
                .label("Re-add")
                .disabled(true),
        ])]))
}

#[derive(Debug)]
pub struct Shop<'a> {
    item: &'a str,
    personal: bool,
    quantity: i64,
    store: Option<&'a str>,
    notes: Option<&'a str>,
}

impl<'a> TryFrom<&'a CommandInteraction> for Shop<'a> {
    type Error = String;
    fn try_from(interaction: &'a CommandInteraction) -> Result<Self, Self::Error> {
        let options = interaction.data.options();

        let mut item: Option<&str> = None;
        let mut personal: Option<bool> = None;
        let mut quantity: Option<i64> = None;
        let mut store: Option<&str> = None;
        let mut notes: Option<&str> = None;

        for option in options.into_iter() {
            match (option.name, option.value) {
                ("item", ResolvedValue::String(val)) => item = Some(val),
                ("personal", ResolvedValue::Boolean(val)) => personal = Some(val),
                ("quantity", ResolvedValue::Integer(val)) => quantity = Some(val),
                ("store", ResolvedValue::String(val)) => store = Some(val),
                ("notes", ResolvedValue::String(val)) => notes = Some(val),
                (opt, val) => {
                    panic!("unexpected option name: `{}` and value `{:?}`", opt, val)
                }
            }
        }

        if item.is_none() || personal.is_none() {
            return Err(String::from("item and personal are required"));
        }
        let item = item.unwrap();
        let personal = personal.unwrap();
        let quantity = quantity.unwrap_or(1);

        Ok(Shop {
            item,
            personal,
            quantity,
            store,
            notes,
        })
    }
}

#[async_trait]
impl<'a> Command<'a> for Shop<'a> {
    fn name() -> &'static str {
        "shop"
    }

    fn description() -> &'static str {
        "add an item to the shopping list"
    }

    fn get_application_command_options(cmd: CreateCommand) -> CreateCommand {
        cmd.add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "item",
                "The item to add to the shopping list",
            )
            .required(true)
            .set_autocomplete(true)
            .max_length(200)
            .to_owned(),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Boolean,
                "personal",
                "true if the item is just for you",
            )
            .required(true),
        )
        .add_option({
            let mut cmd = CreateCommandOption::new(
                CommandOptionType::Integer,
                "quantity",
                "The quantity of the item to add to the shopping list",
            )
            .required(false);

            for i in 1..26 {
                cmd = cmd.add_int_choice(i.to_string(), i);
            }
            cmd
        })
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "store",
                "If the item is to be bought or found in a particular store",
            )
            .required(false)
            .set_autocomplete(true)
            .max_length(100)
            .to_owned(),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "notes",
                "Notes about the item to add to the shopping list",
            )
            .required(false)
            .max_length(100)
            .to_owned(),
        )
    }

    async fn handle_application_command<'b>(
        self,
        interaction: &'b CommandInteraction,
        state: &'b AppState,
        ctx: &'b Context,
    ) -> Result<CommandResponse, CommandResponse> {
        let loading_message = create_loading_message(interaction, ctx).await?;
        let resp = create_new_shopping(&self).await?;

        if let Err(e) = interaction.create_followup(&ctx, resp).await {
            error!("error creating followup: {}", e);
            return Err(CommandResponse::NoResponse);
        }

        push_list_item_to_database(self, state, interaction, ctx, loading_message).await?;

        Ok(CommandResponse::NoResponse)
    }
}

#[async_trait]
impl<'a> AutocompleteCommand<'a> for Shop<'a> {
    async fn autocomplete<'c>(
        command: &'c CommandInteraction,
        autocomplete: &'c AutocompleteOption,
        app_state: &'c AppState,
        _: &'c Context,
    ) -> Result<CreateAutocompleteResponse, CommandResponse> {
        let mut response = CreateAutocompleteResponse::new();
        let user_id: u64 = command.user.id.into();

        let mut items = match app_state
            .get_recent_shopping_list_items_by_user(user_id, 50)
            .await
        {
            Ok(items) => items,
            Err(e) => {
                return Err(CommandResponse::InternalFailure(format!(
                    "error communicating with database: {}",
                    e
                )));
            }
        };

        let extra_items = match app_state.get_recent_shopping_list_items(50).await {
            Ok(items) => items,
            Err(e) => {
                return Err(CommandResponse::InternalFailure(format!(
                    "error communicating with database: {}",
                    e
                )));
            }
        };

        for item in extra_items {
            if !items.contains(&item) {
                items.push(item);
            }
        }

        let search_phrase = autocomplete.value;

        match autocomplete.name {
            "item" => {
                let mut item_names: HashSet<String> =
                    items.into_iter().map(|item| item.item).collect();
                item_names.extend(EXTRA_ITEMS.iter().map(|item| item.to_string()));

                //sort item names, preferring items that start with, then contain, the current search phrase
                let mut item_names: Vec<String> = item_names.into_iter().collect();
                item_names.sort_by(|a, b| {
                    let a_start = a.starts_with(search_phrase);
                    let b_start = b.starts_with(search_phrase);
                    let a_contains = a.contains(search_phrase);
                    let b_contains = b.contains(search_phrase);

                    if a_start && !b_start {
                        Ordering::Less
                    } else if !a_start && b_start {
                        Ordering::Greater
                    } else if a_contains && !b_contains {
                        Ordering::Less
                    } else if !a_contains && b_contains {
                        Ordering::Greater
                    } else {
                        a.cmp(b)
                    }
                });
                item_names.truncate(25);

                let choices: Vec<AutocompleteChoice> = item_names
                    .into_iter()
                    .map(|item| AutocompleteChoice {
                        name: item.clone(),
                        value: serde_json::Value::String(item),
                    })
                    .collect();

                response = response.set_choices(choices);
            }
            "store" => {
                let mut store_names: HashSet<String> =
                    items.into_iter().filter_map(|item| item.store).collect();
                store_names.extend(EXTRA_STORE_NAMES.iter().map(|store| store.to_string()));

                //sort store names, preferring stores that start with, then contain, the current search phrase
                let mut store_names: Vec<String> = store_names.into_iter().collect();
                store_names.sort_by(|a, b| {
                    let a_start = a.starts_with(search_phrase);
                    let b_start = b.starts_with(search_phrase);
                    let a_contains = a.contains(search_phrase);
                    let b_contains = b.contains(search_phrase);

                    if a_start && !b_start {
                        Ordering::Less
                    } else if !a_start && b_start {
                        Ordering::Greater
                    } else if a_contains && !b_contains {
                        Ordering::Less
                    } else if !a_contains && b_contains {
                        Ordering::Greater
                    } else {
                        a.cmp(b)
                    }
                });
                store_names.truncate(25);

                let choices: Vec<AutocompleteChoice> = store_names
                    .into_iter()
                    .map(|store| AutocompleteChoice {
                        name: store.clone(),
                        value: serde_json::Value::String(store),
                    })
                    .collect();

                response = response.set_choices(choices);
            }
            _ => {
                return Err(CommandResponse::InternalFailure(
                    "Invalid autocomplete option".to_string(),
                ));
            }
        }

        Ok(response)
    }
}

#[async_trait]
impl<'a> InteractionCommand<'a> for Shop<'a> {
    async fn answerable<'b>(
        interaction: &'b ComponentInteraction,
        app_state: &'b AppState,
        _: &'b Context,
    ) -> bool {
        let msg_id: u64 = interaction.message.id.into();
        match app_state.get_shopping_list_item_by_message_id(msg_id).await {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                error!("error communicating with database: {}", e);
                false
            }
        }
    }

    async fn interaction<'b>(
        interaction: &'b ComponentInteraction,
        app_state: &'b AppState,
        ctx: &'b Context,
    ) -> Result<CommandResponse, CommandResponse> {
        let msg_id: u64 = interaction.message.id.into();
        let user_id: u64 = interaction.user.id.into();

        match interaction.data.custom_id.as_ref() {
            "bought" => {
                if let Err(e) = app_state
                    .set_shopping_list_item_bought(user_id, msg_id, true)
                    .await
                {
                    return Err(CommandResponse::InternalFailure(format!(
                        "error communicating with database: {}",
                        e
                    )));
                }

                let ex_embed = match interaction.message.embeds.get(0) {
                    Some(embed) => embed,
                    None => {
                        return Err(CommandResponse::InternalFailure(
                            "error communicating with discord".to_string(),
                        ));
                    }
                };

                let mut edit_message = interaction.message.clone();

                if let Err(e) = edit_message
                    .edit(
                        &ctx,
                        EditMessage::new()
                            .embed(
                                CreateEmbed::new()
                                    //XXX: title?
                                    .description(format!(
                                        "(BOUGHT) ~~{}~~",
                                        ex_embed
                                            .description
                                            .as_ref()
                                            .expect("description not found")
                                    ))
                                    .color(EmbedColor::Green as u32),
                            )
                            .components(vec![CreateActionRow::Buttons(vec![CreateButton::new(
                                "readd",
                            )
                            .style(serenity::all::ButtonStyle::Secondary)
                            .label("Re-add")
                            .disabled(false)])]),
                    )
                    .await
                {
                    return Err(CommandResponse::InternalFailure(format!(
                        "error communicating with discord: {}",
                        e
                    )));
                }

                interaction
                    .create_response(&ctx, CreateInteractionResponse::Acknowledge)
                    .await
                    .unwrap();
            }
            "remove" => {
                // mark as bought in database
                if let Err(e) = app_state
                    .set_shopping_list_item_bought(user_id, msg_id, true)
                    .await
                {
                    return Err(CommandResponse::InternalFailure(format!(
                        "error communicating with database: {}",
                        e
                    )));
                }

                let ex_embed = match interaction.message.embeds.get(0) {
                    Some(embed) => embed,
                    None => {
                        return Err(CommandResponse::InternalFailure(
                            "error communicating with discord".to_string(),
                        ));
                    }
                };

                let mut edit_message = interaction.message.clone();

                if let Err(e) = edit_message
                    .edit(
                        &ctx,
                        EditMessage::new()
                            .embed(
                                CreateEmbed::new()
                                    .color(EmbedColor::Orange as u32)
                                    .description(format!(
                                        "(REMOVED) {}",
                                        ex_embed
                                            .description
                                            .as_ref()
                                            .expect("description not found")
                                    )),
                            )
                            .components(vec![CreateActionRow::Buttons(vec![CreateButton::new(
                                "readd",
                            )
                            .style(serenity::all::ButtonStyle::Secondary)
                            .label("Re-add")
                            .disabled(false)])]),
                    )
                    .await
                {
                    return Err(CommandResponse::InternalFailure(format!(
                        "error communicating with discord: {}",
                        e
                    )));
                }

                interaction
                    .create_response(&ctx, CreateInteractionResponse::Acknowledge)
                    .await
                    .unwrap();
            }
            "readd" => {
                let item = match app_state.get_shopping_list_item_by_message_id(msg_id).await {
                    Ok(Some(item)) => item,
                    Ok(None) => {
                        return Err(CommandResponse::InternalFailure(
                            "error communicating with database".to_string(),
                        ));
                    }
                    Err(e) => {
                        return Err(CommandResponse::InternalFailure(format!(
                            "error communicating with database: {}",
                            e
                        )));
                    }
                };

                create_loading_message(interaction, ctx).await?;
                let shop = Shop {
                    item: item.item.as_ref(),
                    personal: item.personal,
                    quantity: item.quantity,
                    store: item.store.as_deref(),
                    notes: item.notes.as_deref(),
                };
                let resp = create_new_shopping(&shop).await?;

                let msg_id = match interaction.create_followup(&ctx, resp).await {
                    Ok(m) => m,
                    Err(e) => {
                        return Err(CommandResponse::InternalFailure(format!(
                            "error communicating with discord: {}",
                            e
                        )));
                    }
                };

                push_list_item_to_database(shop, app_state, interaction, ctx, msg_id.id.into())
                    .await?;
            }
            _ => {
                return Err(CommandResponse::InternalFailure(
                    "Invalid interaction".to_string(),
                ));
            }
        }

        Ok(CommandResponse::NoResponse)
    }
}

// pub struct ShoppingComplete;

// impl<'a> TryFrom<&'a CommandInteraction> for ShoppingComplete {
//     type Error = String;

//     fn try_from(_: &'a CommandInteraction) -> Result<Self, Self::Error> {
//         Ok(ShoppingComplete)
//     }
// }

// #[async_trait]
// impl<'a> Command<'a> for ShoppingComplete {
//     fn name() -> &'static str {
//         "shopping-complete"
//     }

//     fn description() -> &'static str {
//         "Run this command once you have completed shopping"
//     }

//     fn get_application_command_options(command: CreateCommand) -> CreateCommand {
//         command
//     }

//     async fn handle_application_command<'b>(
//         self,
//         cmd_interaction: &'b CommandInteraction,
//         app_state: &'b AppState,
//         ctx: &'b Context,
//     ) -> Result<CommandResponse, CommandResponse> {
//         // TODO: actually make use of the shopping list -> shopping list item table
//         // to separate what items are actually available to be bought when this command runs
//         if let Err(e) = cmd_interaction.create_response(&ctx,
//             CreateInteractionResponse::Message(
//                 CreateInteractionResponseMessage::new()
//                     .content("-----------------------------------\n**Shopping Complete!**\n-----------------------------------")
//             )
//         ).await {
//             error!("error communicating with discord to create initial response: {}", e);
//             return Err(CommandResponse::InternalFailure(format!(
//                 "error communicating with discord: {}",
//                 e
//             )));
//         }

//         // collect every non-bought item from the shopping list
//         let items = match app_state.get_unbought_shopping_list_items().await {
//             Ok(items) => items,
//             Err(e) => {
//                 return Err(CommandResponse::InternalFailure(format!(
//                     "error communicating with database: {}",
//                     e
//                 )));
//             }
//         };

//         let channel = cmd_interaction.channel_id();

//         // for each item, send a message to the shopping channel
//         for item in items {
//             let shop = Shop {
//                 item: item.item.as_ref(),
//                 personal: item.personal,
//                 quantity: item.quantity,
//                 store: item.store.as_deref(),
//                 notes: item.notes.as_deref(),
//             };

//             let resp = create_new_shopping(&shop).await?;

//             let new_msg = match channel.send_message(&ctx, resp).await {
//                 Ok(m) => m,
//                 Err(e) => {
//                     error!(
//                         "error communicating with discord to send shopping list item: {}",
//                         e
//                     );
//                     return Err(CommandResponse::InternalFailure(format!(
//                         "error communicating with discord: {}",
//                         e
//                     )));
//                 }
//             };

//             push_list_item_to_database(shop, app_state, cmd_interaction, ctx, new_msg.id.into())
//                 .await?;

//             // mark old item as bought in the database
//             if let Err(e) = app_state
//                 .set_shopping_list_item_bought(item.user_id as u64, item.message_id as u64, true)
//                 .await
//             {
//                 return Err(CommandResponse::InternalFailure(format!(
//                     "error communicating with database: {}",
//                     e
//                 )));
//             }

//             let ex_embed = match channel.message(&ctx, item.message_id as u64).await {
//                 Ok(m) => m.embeds.first().unwrap().clone(),
//                 Err(e) => {
//                     error!("error communicating with discord to get old message: {}", e);
//                     return Err(CommandResponse::InternalFailure(format!(
//                         "error communicating with discord: {}",
//                         e
//                     )));
//                 }
//             };

//             // edit the old message to show that it has been refreshed
//             if let Err(e) = channel
//                 .edit_message(
//                     ctx,
//                     item.message_id as u64,
//                     EditMessage::new()
//                         .embed(
//                             CreateEmbed::new()
//                                 .description(format!(
//                                     "(REFRESHED) ~~{}~~",
//                                     ex_embed
//                                         .description
//                                         .as_ref()
//                                         .expect("description not found")
//                                 ))
//                                 .color(EmbedColor::Blue as u32),
//                         )
//                         .components(vec![]),
//                 )
//                 .await
//             {
//                 error!(
//                     "error communicating with discord to edit old message: {}",
//                     e
//                 );
//                 return Err(CommandResponse::InternalFailure(format!(
//                     "error communicating with discord to edit old message: {}",
//                     e
//                 )));
//             }
//         }

//         Ok(CommandResponse::NoResponse)
//     }
// }
