package com.cipher.social

import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.action.ViewActions.*
import androidx.test.espresso.assertion.ViewAssertions.matches
import androidx.test.espresso.matcher.ViewMatchers.*
import androidx.test.espresso.web.webdriver.Locator
import androidx.test.espresso.web.webdriver.DriverAtoms.*
import androidx.test.espresso.web.assertion.WebViewAssertions.webMatches
import androidx.test.espresso.web.sugar.Web.onWebView
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class AccountCreationTest {

    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    @Test
    fun testAccountCreationFlow() {
        // Wait for the app to load
        Thread.sleep(5000)

        // Wait for the web view to load and navigate to account creation
        onWebView()
            .withElement(findElement(Locator.LINK_TEXT, "Create Account"))
            .perform(webClick())

        // Wait for navigation
        Thread.sleep(2000)

        // Fill in the account creation form
        onWebView()
            .withElement(findElement(Locator.NAME, "user[username]"))
            .perform(webKeys("testuser"))

        onWebView()
            .withElement(findElement(Locator.NAME, "user[display_name]"))
            .perform(webKeys("Test User"))

        onWebView()
            .withElement(findElement(Locator.NAME, "user[email]"))
            .perform(webKeys("test@example.com"))

        onWebView()
            .withElement(findElement(Locator.NAME, "user[password]"))
            .perform(webKeys("securepass123"))

        onWebView()
            .withElement(findElement(Locator.NAME, "user[password_confirmation]"))
            .perform(webKeys("securepass123"))

        // Submit the form
        onWebView()
            .withElement(findElement(Locator.CSS_SELECTOR, "input[type='submit'], button[type='submit']"))
            .perform(webClick())

        // Wait for submission and check for success
        Thread.sleep(3000)

        // Verify we're redirected to the dashboard or see success message
        onWebView()
            .withElement(findElement(Locator.TAG_NAME, "body"))
            .check(webMatches(getText(), containsString("Welcome to Cipher")))
    }

    @Test
    fun testAccountCreationFormValidation() {
        // Wait for the app to load
        Thread.sleep(5000)

        // Navigate to account creation
        onWebView()
            .withElement(findElement(Locator.LINK_TEXT, "Create Account"))
            .perform(webClick())

        // Wait for navigation
        Thread.sleep(2000)

        // Try to submit form without filling required fields
        onWebView()
            .withElement(findElement(Locator.CSS_SELECTOR, "input[type='submit'], button[type='submit']"))
            .perform(webClick())

        // Wait for validation messages
        Thread.sleep(1000)

        // Check that we're still on the form page (validation failed)
        onWebView()
            .withElement(findElement(Locator.TAG_NAME, "body"))
            .check(webMatches(getText(), containsString("Username")))
    }
}