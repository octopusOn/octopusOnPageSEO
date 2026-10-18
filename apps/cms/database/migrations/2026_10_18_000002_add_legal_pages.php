<?php

use App\Enums\PageType;
use App\Models\Page;
use Database\Seeders\LegalSeeder;
use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Legal texts of the account site (my.octopuson.com) become `legal` pages, found by a key that
 * stays the same in every language (`privacy`, `distance-sales`, …).
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::table('pages', function (Blueprint $table) {
            $table->string('key', 40)->nullable()->unique()->after('type');
        });

        (new LegalSeeder)->run();
    }

    public function down(): void
    {
        Page::query()->where('type', PageType::Legal)->get()->each->delete();
        Schema::table('pages', function (Blueprint $table) {
            $table->dropUnique(['key']);
            $table->dropColumn('key');
        });
    }
};
