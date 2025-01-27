cmp="$UTILS/cmp.sh"

echo "Creating a nested directory structure"

mkdir -p /some/nested/directory/structure

echo "Testing the stack, should be empty"

b -l 1 | $cmp "" "empty backward stack"
f -l 1 | $cmp "" "empty forward stack"
b -l 8 | $cmp "" "still empty backward stack"
f -l 8 | $cmp "" "still empty forward stack"

echo "Testing the stack with one step history"

fst_dir=$(pwd)

safe_cd /some/nested/directory/structure
pwd | $cmp "/some/nested/directory/structure" "check current directory"
b -l 1 | $cmp "$fst_dir" "backward stack with one element"
f -l 1 | $cmp "" "empty forward stack"
b -l 8 | $cmp "$fst_dir" "still backward stack with one element"
f -l 8 | $cmp "" "still empty forward stack"

echo "Testing the stack with two step history"

safe_cd /some/nested
pwd | $cmp "/some/nested" "check current directory"
b -l 2 | $cmp "$fst_dir\n/some/nested/directory/structure\n" "backward stack with two elements"
f -l 2 | $cmp "" "empty forward stack"
b -l 8 | $cmp "$fst_dir\n/some/nested/directory/structure\n" \
    "still backward stack with two elements"
f -l 8 | $cmp "" "still empty forward stack"

echo "Testing the stack with three step history"

safe_cd /some
pwd | $cmp "/some" "check current directory"
b -l 3 | $cmp "$fst_dir\n/some/nested/directory/structure\n/some/nested\n" \
    "backward stack with three elements"
f -l 3 | $cmp "" "empty forward stack"
b -l 8 | $cmp "$fst_dir\n/some/nested/directory/structure\n/some/nested\n" \
    "still backward stack with three elements"
f -l 8 | $cmp "" "still empty forward stack"

echo "Go back"

b
pwd | $cmp "/some/nested" "check current directory"
b -l 2 | $cmp "$fst_dir\n/some/nested/directory/structure\n" "backward stack with two elements"
f -l 1 | $cmp "/some\n" "forward stack with one element"
b -l 8 | $cmp "$fst_dir\n/some/nested/directory/structure\n" \
    "still backward stack with two elements"
f -l 8 | $cmp "/some\n" "still forward stack with one element"

echo "Go back again"

b
pwd | $cmp "/some/nested/directory/structure" "check current directory"
b -l 1 | $cmp "$fst_dir\n" "backward stack with one element"
f -l 2 | $cmp "/some\n/some/nested\n" "forward stack with two elements"
b -l 8 | $cmp "$fst_dir\n" "still backward stack with one element"
f -l 8 | $cmp "/some\n/some/nested\n" "still forward stack with two elements"

echo "Go forward"

f
pwd | $cmp "/some/nested" "check current directory"
b -l 2 | $cmp "$fst_dir\n/some/nested/directory/structure\n" "backward stack with two elements"
f -l 1 | $cmp "/some\n" "forward stack with one element"
b -l 8 | $cmp "$fst_dir\n/some/nested/directory/structure\n" \
    "still backward stack with two elements"
f -l 8 | $cmp "/some\n" "still forward stack with one element"

echo "Go forward again"

f
pwd | $cmp "/some" "check current directory"
b -l 3 | $cmp "$fst_dir\n/some/nested/directory/structure\n/some/nested\n" \
    "backward stack with three elements"
f -l 1 | $cmp "" "empty forward stack"
b -l 8 | $cmp "$fst_dir\n/some/nested/directory/structure\n/some/nested\n" \
    "still backward stack with three elements"
f -l 8 | $cmp "" "still empty forward stack"

echo "Go back multiple times"

b 2
pwd | $cmp "/some/nested/directory/structure" "check current directory"
b -l 1 | $cmp "$fst_dir\n" "backward stack with one element"
f -l 2 | $cmp "/some\n/some/nested\n" "forward stack with two elements"
b -l 8 | $cmp "$fst_dir\n" "still backward stack with one element"
f -l 8 | $cmp "/some\n/some/nested\n" "still forward stack with two elements"

echo "Go forward multiple times"

f 2
pwd | $cmp "/some" "check current directory"
b -l 3 | $cmp "$fst_dir\n/some/nested/directory/structure\n/some/nested\n" \
    "backward stack with three elements"
f -l 1 | $cmp "" "empty forward stack"
b -l 8 | $cmp "$fst_dir\n/some/nested/directory/structure\n/some/nested\n" \
    "still backward stack with three elements"
f -l 8 | $cmp "" "still empty forward stack"

echo "Go back more than the history size"

b 10
pwd | $cmp "$fst_dir" "check current directory"
b -l 1 | $cmp "" "empty backward stack"
f -l 3 | $cmp "/some\n/some/nested\n/some/nested/directory/structure\n" \
    "forward stack with three elements"
b -l 8 | $cmp "" "still empty backward stack"
f -l 8 | $cmp "/some\n/some/nested\n/some/nested/directory/structure\n" \
    "still forward stack with three elements"

echo "Go forward more than the history size"

f 10
pwd | $cmp "/some" "check current directory"
b -l 3 | $cmp "$fst_dir\n/some/nested/directory/structure\n/some/nested\n" \
    "backward stack with three elements"
f -l 1 | $cmp "" "empty forward stack"
b -l 8 | $cmp "$fst_dir\n/some/nested/directory/structure\n/some/nested\n" \
    "still backward stack with three elements"
f -l 8 | $cmp "" "still empty forward stack"
