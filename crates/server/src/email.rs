use crate::db::EmailConfig;
use lettre::{
    message::header::ContentType,
    transport::smtp::{authentication::Credentials, client::Tls},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use tracing::info;

pub async fn send_failure_email(
    config: &EmailConfig,
    user_email: Option<&str>,
    error_detail: &str,
    task_description: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let body = format!(
        "A background task has failed.\n\n\
         Task: {task_description}\n\
         Error: {error_detail}\n\n\
         A support ticket has been created. You can add additional information \
         by visiting the tickets page in VenueInter.\n\n\
         If you need immediate assistance, please contact the system administrator."
    );

    let mut recipients = vec![config.sysadmin_email.clone()];
    if let Some(email) = user_email {
        if !email.is_empty() && email != config.sysadmin_email {
            recipients.push(email.to_string());
        }
    }

    for recipient in &recipients {
        let message = Message::builder()
            .from(config.from_address.parse()?)
            .to(recipient.parse()?)
            .subject(format!("VenueInter Task Failed: {task_description}"))
            .header(ContentType::TEXT_PLAIN)
            .body(body.clone())?;

        let mut transport_builder =
            AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)?
                .port(config.smtp_port);

        if let (Some(user), Some(pass)) = (&config.smtp_user, &config.smtp_password) {
            transport_builder =
                transport_builder.credentials(Credentials::new(user.clone(), pass.clone()));
        }

        if !config.smtp_tls {
            transport_builder = transport_builder.tls(Tls::None);
        }

        let transport = transport_builder.build();
        transport.send(message).await?;
        info!(recipient, "Failure email sent");
    }

    Ok(())
}
