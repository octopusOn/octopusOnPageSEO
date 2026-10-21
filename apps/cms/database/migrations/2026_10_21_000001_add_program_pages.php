<?php

use Database\Seeders\ProgramSeeder;
use Illuminate\Database\Migrations\Migration;

/**
 * Pages of programs added after the website's first content (database/seeders/programs/), as
 * drafts: the GEO-SEO Platform first.
 */
return new class extends Migration
{
    public function up(): void
    {
        (new ProgramSeeder)->run();
    }

    public function down(): void
    {
        // The pages may have been edited and published since; they are removed in the admin.
    }
};
