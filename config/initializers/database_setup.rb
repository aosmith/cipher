# Database setup for packaged applications (desktop, mobile)
# This ensures database is created and migrated when Rails starts

if Rails.env.desktop? || Rails.env.ios? || Rails.env.android?
  Rails.application.config.after_initialize do
    begin
      puts "Starting database initialization for #{Rails.env}..."
      puts "Database URL: #{ENV['DATABASE_URL']}" if ENV['DATABASE_URL']
      puts "Database config: #{ActiveRecord::Base.connection_db_config.database}"

      # First ensure the database file exists in the correct location
      # In bundled apps, use the app data directory, not the bundled resources
      if ENV['DATABASE_URL']
        db_path = ENV['DATABASE_URL'].gsub('sqlite3://', '')
      else
        db_path = ActiveRecord::Base.connection_db_config.database
      end

      db_dir = File.dirname(db_path)

      unless Dir.exist?(db_dir)
        puts "Creating database directory: #{db_dir}"
        FileUtils.mkdir_p(db_dir)
      end

      # Check if database exists and has tables
      unless ActiveRecord::Base.connection.data_source_exists?('users')
        puts "Database not initialized. Setting up fresh database..."

        # Try schema load first, then fallback to migrations
        schema_path = Rails.root.join('db', 'schema.rb')
        if File.exist?(schema_path)
          puts "Loading schema from #{schema_path}..."
          load(schema_path)
          puts "Schema loaded successfully."
        else
          puts "Schema file not found, running migrations..."
          ActiveRecord::MigrationContext.new(Rails.root.join('db', 'migrate')).migrate
          puts "Migrations completed."
        end

        puts "Database initialization complete!"
      else
        puts "Database already initialized - users table exists."
      end
    rescue ActiveRecord::NoDatabaseError => e
      puts "Database doesn't exist: #{e.message}"
      puts "Creating database and setting up schema..."

      # Create directory if needed
      db_path = ActiveRecord::Base.connection_db_config.database
      db_dir = File.dirname(db_path)
      FileUtils.mkdir_p(db_dir) unless Dir.exist?(db_dir)

      # Create the database file
      ActiveRecord::Tasks::DatabaseTasks.create_current
      puts "Database created."

      # Load schema or run migrations
      schema_path = Rails.root.join('db', 'schema.rb')
      if File.exist?(schema_path)
        puts "Loading schema..."
        load(schema_path)
        puts "Schema loaded."
      else
        puts "Running migrations..."
        ActiveRecord::MigrationContext.new(Rails.root.join('db', 'migrate')).migrate
        puts "Migrations completed."
      end

      puts "Database initialization complete!"
    rescue => e
      puts "Database setup error: #{e.class}: #{e.message}"
      puts "Backtrace: #{e.backtrace.first(5).join("\n")}"

      # Last resort: try to create tables manually
      puts "Attempting manual table creation..."
      begin
        ActiveRecord::MigrationContext.new(Rails.root.join('db', 'migrate')).migrate
        puts "Manual migration completed."
      rescue => migration_error
        puts "Migration error: #{migration_error.class}: #{migration_error.message}"
        puts "Database setup failed completely."
      end
    end
  end
end