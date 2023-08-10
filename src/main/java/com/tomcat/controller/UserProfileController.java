package com.tomcat.controller;

import com.tomcat.domain.UserProfile;
import com.tomcat.service.UserProfileService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/profile")
public class UserProfileController {

  @Autowired
  private UserProfileService userProfileService;

  @GetMapping("/get/{userId}")
  public ResponseEntity<UserProfile> getByUserId(@PathVariable Long userId) {
    UserProfile profile = userProfileService.getByUserId(userId);
    return ResponseEntity.ok(profile); 
  }  

  @PutMapping("/update")
  public ResponseEntity<UserProfile> updateProfile(@RequestBody UserProfile profile) {
    UserProfile updatedProfile = userProfileService.update(profile);
    return ResponseEntity.ok(updatedProfile);
  }

}