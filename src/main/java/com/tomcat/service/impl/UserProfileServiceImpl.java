package com.tomcat.service.impl;

import com.tomcat.domain.UserProfile;
import com.tomcat.domain.UserProfileRepository;
import com.tomcat.service.UserProfileService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;

@Service
public class UserProfileServiceImpl implements UserProfileService {
  
  @Autowired
  private UserProfileRepository userProfileRepository;

  @Override
  public UserProfile getByUserId(Long userId) {
    return userProfileRepository.findByUserId(userId); 
  }

  @Override
  public UserProfile update(UserProfile profile) {
    return userProfileRepository.save(profile);
  }

}