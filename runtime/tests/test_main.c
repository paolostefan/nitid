#include "nitid_test.h"

TEST_GLOBALS

void register_array_tests(void);
void register_string_tests(void);
void register_string16_tests(void);
void register_string32_tests(void);

int main(void) {
    printf("NITID Runtime Test Suite\n");
    printf("=======================\n\n");

    printf("[nitid_array]\n");
    register_array_tests();

    printf("\n[nitid_string]\n");
    register_string_tests();

    printf("\n[nitid_string16]\n");
    register_string16_tests();

    printf("\n[nitid_string32]\n");
    register_string32_tests();

    return test_summary();
}
