use serenity::{
    builder::{CreateApplicationCommand, CreateApplicationCommands},
    model::prelude::command::CommandOptionType,
};

pub fn create_all(commands: &mut CreateApplicationCommands) -> &mut CreateApplicationCommands {
    commands
        .create_application_command(|command| stats(command))
        .create_application_command(|command| ranking(command))
        .create_application_command(|command| gesagt(command))
        .create_application_command(|command| assistiert(command))
        .create_application_command(|command| fertig(command))
}

fn stats(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("stats")
        .description("Erhalte Statistiken von jemandem")
        .create_option(|option| {
            option
                .name("name")
                .description("Der, von dem du die Statistiken willst")
                .kind(CommandOptionType::String)
        })
}

fn ranking(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("ranking")
        .description("Rankt alle Mitglieder nach der Anzahl ihrer gesagten, assistierten oder geschriebenen Zitate")
        .create_option(|option| option
                       .name("kategorie")
                       .description("Die Kategorie, nach der du ranken willst")
                       .kind(CommandOptionType::String)
                       .required(true)
                       .add_string_choice("gesagt", "said")
                       .add_string_choice("geschrieben", "wrote")
                       .add_string_choice("assistiert", "assisted"))
}

fn gesagt(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("gesagt")
        .description("Fügt einen Zitierten zum Zitat hinzu")
        .create_option(|option| {
            option
                .name("name")
                .description("Der, der das Zitat gesagt hat")
                .kind(CommandOptionType::String)
                .required(true)
        })
}

fn assistiert(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("assistiert")
        .description("Fügt einen Assister zum Zitat hinzu")
        .create_option(|option| {
            option
                .name("name")
                .description("Der, der einen Assist gemacht hat")
                .kind(CommandOptionType::String)
                .required(true)
        })
}

fn fertig(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("fertig")
        .description("Alle Sager und Assister sind eingetragen; Thread wird gelöscht")
}
