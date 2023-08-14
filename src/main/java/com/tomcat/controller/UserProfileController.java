package com.tomcat.controller;

import com.tomcat.controller.requeset.ProfileResquest;
import com.tomcat.controller.response.ProfileResponse;
import com.tomcat.service.UserProfileService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/profile")
public class UserProfileController {

  @Autowired
  private UserProfileService userProfileService;

  @GetMapping("/getProfile")
  public ResponseEntity<ProfileResponse> getByUserId(@RequestParam String userId) {
    ProfileResponse profile = userProfileService.getByUserId(userId);
    return ResponseEntity.ok(profile); 
  }  

  @PutMapping("/update")
  public ResponseEntity<ProfileResponse> updateProfile(@RequestBody ProfileResquest profileDTO) {
    ProfileResponse profile = userProfileService.update(profileDTO);
    return ResponseEntity.ok(profile);
  }

}