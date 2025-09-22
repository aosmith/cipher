use tauri_driver::{Driver, By};
use std::time::Duration;

#[tokio::test]
async fn test_desktop_app_launches() -> Result<(), Box<dyn std::error::Error>> {
    // Start the Tauri app
    let driver = Driver::new().await?;

    // Wait for app to initialize
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Check that the app window is present
    let title = driver.title().await?;
    assert_eq!(title, "Cipher");

    // Verify the main page loads
    let cipher_text = driver.find_element(By::Css("body")).await?;
    let body_text = cipher_text.text().await?;
    assert!(body_text.contains("Cipher"));

    driver.quit().await?;
    Ok(())
}

#[tokio::test]
async fn test_desktop_app_account_creation() -> Result<(), Box<dyn std::error::Error>> {
    let driver = Driver::new().await?;

    // Wait for app to load
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Navigate to account creation
    let create_account_link = driver.find_element(By::LinkText("Create Account")).await?;
    create_account_link.click().await?;

    // Wait for navigation
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Fill in the form
    let username_field = driver.find_element(By::Css("input[name='user[username]']")).await?;
    username_field.send_keys("testuser").await?;

    let display_name_field = driver.find_element(By::Css("input[name='user[display_name]']")).await?;
    display_name_field.send_keys("Test User").await?;

    let password_field = driver.find_element(By::Css("input[name='password']")).await?;
    password_field.send_keys("securepass123").await?;

    let confirm_password_field = driver.find_element(By::Css("input[name='confirm_password']")).await?;
    confirm_password_field.send_keys("securepass123").await?;

    // Submit the form
    let submit_button = driver.find_element(By::Css("input[type='submit']")).await?;
    submit_button.click().await?;

    // Wait for submission
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Verify successful account creation
    let body = driver.find_element(By::Css("body")).await?;
    let body_text = body.text().await?;
    assert!(body_text.contains("Welcome to Cipher"));

    driver.quit().await?;
    Ok(())
}